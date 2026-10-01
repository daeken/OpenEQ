# Native sky lighting colors and day sampling

2026-10-01. Native research and bounded static day-key sampling in the loader.
The native sky's reserved color-map column supplies environment lighting.
Those entries must remain excluded from visible dome sampling, but they are
not disposable padding. Both the dome and lighting getters use the same
current, interpolated 32×32 color table.

## Provenance and execution boundary

Original installed `EQGraphicsDX9.dll`, image base `0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Host `eqgame.exe`, image base `0x00400000`, SHA-256
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
Host routine `0x004b9440` supplies time through sky slot `+0x48`, selects
sun/moon through `+0x4c`, and requests angle/color/bounce through the slots
below. Host transfer and final shader use are independent research boundaries.

The executable witness `/tmp/openeq-sky-light-inputs.py` runs original x86
instructions under Unicorn, x87 CW `0x037f`, MXCSR `0x1f80`. It executes:

- Five color getters and the weather-manager table-pointer getter.
- Weather-manager update `0x100331c0`, including its native pattern-update
  call with external weather controller absent.
- Day sampling `0x1002dfe0 → 0x1002ebf0 → 0x1002eae0` and whole-table blend
  `0x1002e890`.
- Native key-list insertion `0x1002ec80` with original INI-derived keys.
- Four distinct weather-countdown cases, separately from day sampling.
- The native time setter, sun/moon switch, and stored-angle getters.

The witness supplies allocations, key/pattern/manager object layout, parsed
INI fields and uncompressed original DDS table bytes. It does not execute
INI/registry/DDS I/O, pattern-name resolution, a native process, graphics
device, server time receipt, or live weather changes. All 1,024 words of each
sample are checked against a separate calculation, not just the five swatches.

## Getter slots and exact table entries

Concrete sky vtable `0x101374dc`; manager pointer is sky `+0x21c`.
`0x10032b60` returns manager `+0x13c4`, the current table pointer.
Coordinates below are `(column, row)` in the original 32×32 DDS.

| Sky slot | Concrete method | Table offset | DDS entry | Host role |
| --- | --- | --- | --- | --- |
| `+0x28` | `0x1002c730` | `0x1fc` | `(31,3)` | Ambient input |
| `+0x30` | `0x1002c7d0` | `0x07c` | `(31,0)` | Sun directional color |
| `+0x34` | `0x1002c810` | `0x0fc` | `(31,1)` | Moon directional color |
| `+0x3c` | `0x1002c8b0` | `0xe7c` | `(31,28)` | Sun bounce color |
| `+0x40` | `0x1002c900` | `0xefc` | `(31,29)` | Moon bounce color |

Each getter copies one packed word unchanged to its own process-global
static and returns a pointer to that static. There is no RGB normalization
or gamma conversion here. On the first call the static initializes to zero.
A missing manager or table subsequently returns the prior cached word,
including across different sky objects; it does not reset to black per call.
The witness tests initial absence, table-present lookup with distinct sentinels,
and missing-manager/table behavior after a successful lookup.

The original DDS masks identify packed `AARRGGBB` values; memory bytes are
BGRA. Example installed `DefaultClear` source entries:

| Source map | Sun directional | Moon directional | Ambient | Sun bounce | Moon bounce |
| --- | --- | --- | --- | --- | --- |
| DefaultDay | `ffffffff` | `ff000000` | `ffdbd9d9` | `ffbdbcbc` | `ffbdbcbc` |
| DefaultNight | `ff000000` | `ff0d56e7` | `ff000000` | `ff04112a` | `ff273757` |
| DefaultDawn | `00feae51` | `00444e75` | `00412838` | `00412838` | `ff412838` |
| DefaultDusk | `00ff9a24` | `00000000` | `005c3e6b` | `00412838` | `ff412838` |

Alpha is retained. Its significance is determined by the eventual consumer;
zero-alpha dawn/dusk words must not be discarded as transparent lighting.
The earlier [dome-domain research](SKY_COLORMAPS.md) remains correct: none of
these column-31 entries belongs in the visible sky gradient.

## Day key timing and byte interpolation

The pattern loader at `0x1003965d..0x10039675` resolves `ColorSet` into pattern
`+0x44`. The manager update reads that field at `0x10033214`, obtains day
fraction through sky slot `+0x44`, and calls the color-set sampler.

The key loader `0x1002e4e0` resolves sequential `ColorMap%u`, `Time%u` and
`Transition%u` fields. At `0x1002e5da` and `0x1002e631`, authored time and
transition values multiply by 65536 and truncate toward zero into integer
fields. The matching sample wrapper does the same to its supplied day fraction
at `0x1002dfeb..0x1002e010`. A key record stores time at `+0x0c`, transition
length at `+0x10`, and color-map payload at `+0x14`; list insertion sorts keys
by time. Actual malformed/duplicate-key policy is outside the executed cases.

For the installed DefaultClear keys and ordinary normalized day fractions,
selection takes the latest key strictly before the sample tick, wrapping to
the last key before the first. Exact equality retains the preceding key;
this also applies to a zero-length transition. `TransitionN` is the interval
**after TimeN** over which the previous
map becomes map N. At elapsed fixed ticks `d`, duration `D`:

```text
if d >= D: use the new map directly
otherwise alpha = floor(d * 255 / D)
if alpha == 0: use the previous map directly
otherwise each packed byte = (new * alpha + previous * (255-alpha)) >> 8
```

This arithmetic interpolates all four bytes of every entry, including lighting
swatches. The denominator is 256, while weights sum to 255; a generic floating
lerp or division by 255 is not byte-identical. The pure blend routine itself
also dims at weights 0/255, but normal day sampling bypasses the blend at the
exact endpoint cases described above.

DefaultClear authored keys, in order, are Dawn at `0.234985` with transition
`0.034988`, Day at `0.279999` with `0.019989`, Dusk at `0.699997` with `0.019989`,
and Night at `0.739990` with `0.029999`. The witness evaluates 17 supplied day
fractions from zero through `0.9999`, checking the complete native output table
against these keys. It also separately executes seven pure blend weights.

## Weather transition is a separate clock

Manager `+0x388` is the current pattern and `+0x38c` the previous pattern.
The manager samples both color sets at the same day fraction. When a previous
pattern exists and remaining transition time `+0x394` is positive, it subtracts
sky delta `+0x20`, divides by total duration `+0x390`, and truncates the result
multiplied by 255. That weight belongs to the **previous** weather pattern;
the new pattern receives `255-weight`. It blends into manager `+0x398` and
publishes that table through `+0x13c4`.

The transition setup multiplies its seconds by 1000 at
`0x10032dfe..0x10032e16`. Sky delta derives from the system millisecond clock
and speed field at `0x100c2d00`; it is not the normalized day fraction. The
manager caches the last byte weight at `+0x13f1`. When the countdown expires
it samples the new pattern directly. Four executed countdown cases distinguish
this behavior from day-key interpolation. A static `load_sky` call with one
explicit day fraction can reproduce the latter without claiming live weather
or clock synchronization.

## Directional-angle boundary

Sky `+0x18` (`0x100c2cc0`) returns the float at sky `+8`; `+0x20`
(`0x100c2ce0`) returns the float at sky `+0x0c`. Their writers and complete
celestial state are not established by this slice.

Time setter `+0x48` (`0x100c2c40`) stores the incoming fraction at `+4` and
writes the dome axis `[0,sin(2*pi*t),cos(2*pi*t)]` at `+0x10`. It leaves the
stored sun/moon angles untouched, as the executable sentinel cases confirm.
Do not substitute that axis for the host's angle input merely because both
vary with sky time.

The switch at `+0x4c` (`0x100c2c70`) chooses the moon branch outside the
inclusive finite interval `[0.2465277761220932, 0.7638888955116272]`, roughly
05:55 through 18:20. The witness checks both boundaries and nearby values.
This predicate alone does not establish satellite motion or final light vector.

## Static loader integration and installed coverage

`load_sky` now samples both the main and Cloud0 color sets at its supplied day
fraction using the proven fixed-tick and byte-blending rules. Existing API
normalization remains explicit: finite fractions use f32 `rem_euclid(1)`,
nonfinite inputs use `0.5`. There is no new live clock, weather-countdown state,
satellite motion or use of these colors for rendered environment lighting.

Every sampled table keeps `OriginalDome` layout. Single-map textures retain
their source filename. Blends have a derived name and separate provenance with
ColorSet, sampled tick, original key ordinals, ColorMap names, resolved source
paths, key times/transitions and byte weight. Both decoded sources remain
unchanged while a new RGBA table is constructed. `raw_light_colors()` exposes
the five original packed AARRGGBB swatches, preserving alpha without applying
host-side lighting policy or shader normalization.

The loader accepts contiguous keys with finite nonnegative time/transition
values below one day, unique fixed ticks, and transitions that finish before
the next key or day end. Unsupported duplicates, gaps and overlapping or
out-of-range intervals are explicit errors. Missing numeric fields default to
zero as in the native parser. Missing sets, map declarations and required
files remain errors; no authored name is silently substituted.

The installed `weather.ini` contains 79 ColorSet sections and 130 keys: 17
four-key sets and 62 single-key sets. Every key is contiguous, finite and
within the accepted timing contract. Lava's authored key order is not sorted;
native sorted insertion and the loader both preserve original key identity
while sampling in time order. The 80 referenced, present DDS files all have
the original 32×32 packed layout. There are two existing declaration holes:
`ColorSet-PoDisease-3` refers to absent `ColorMap-PoDisease-3`, and
`Cloud-lava-0` refers to absent `ColorSet-BrightOverCastCloud0`.

The separate `/tmp/openeq-sky-all-sets.py` executes all 78 resolvable sets at
1,131 full-table samples spanning exact key ticks, neighboring ticks, transition
interiors/endpoints and the day edges. Every sample matches the independent
calculation. The Rust original-asset test compares those same explicit ticks
against native-output CRC32 aggregates retained in
`crates/openeq-assets/src/environment/native_color_map_samples.txt`. The fixture
contains no original pixel bytes. Its generator preserves RGBA channel order
when hashing native BGRA output and includes each little-endian tick before
the corresponding 4,096 output bytes.

Six focused sampling tests cover that original corpus, byte rounding and alpha,
source provenance, main/cloud loading, raw swatches, time normalization, single
maps, malformed declarations, and the native strict zero-transition boundary.
The zero-boundary supplement executes 15 cases using zero-, one- and long-tick
transition durations. All ten environment/atmosphere tests pass with
`CARGO_INCREMENTAL=0 cargo test -p openeq-assets --lib environment -- --include-ignored`.

## Reproduction

Run `PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-sky-light-inputs.py`.
Result `/tmp/openeq-sky-light-inputs.json` includes exact original DDS hashes,
key fields, output colors and executed entry counts. All assertions pass:
17 full-table day samples, seven blend weights, seven time/angle cases and
four weather-countdown cases.

| Artifact | SHA-256 |
| --- | --- |
| Probe `.py` | `e3ae51a767b68089f77e1a32faac07595d22b38b7210dd176fe6c09a72f40adb` |
| Result `.json` | `70c765094f221b596001470ea52fc75a7c3d19dd1cacebaefecac76de26b1bcb` |
| Installed-set probe `.py` | `eb5592fd75f1f9a9b5e9147739b8bd38b626a31e7c616dcf6bfdc10549983b5b` |
| Installed-set result `.json` | `73c2911f0641a11e76eab7e386e78e8f3e13c28c0cbccac09f94b7697c54cf13` |
| CRC fixture generator `.py` | `4f4f27f99e23e8742e1a66be3099bf3826e6714b4675fdba5e26de1e1d71c574` |
| Zero-boundary probe `.py` | `4d2fb4bf211c353d49927507223d3424a070ff7be0d0737a327321ff0de85e18` |
| Zero-boundary result `.json` | `81b709bea06f9bc6d37a753697e8ceffe57cef2fd9cbe602ae45c0271d25e948` |
