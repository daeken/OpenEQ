# Missing authored sky patterns: October 1 follow-up

Thundercrest's installed `DefaultWeather=clz-0` does not identify a declared
weather pattern. Read-only inspection found no basis for substituting
`BrightOverCast`, even though that pattern uses color maps named `clz*`.
The loader continues reporting the unresolved reference. Native selection
failure is a no-op at the weather-manager boundary; the sky visible after a
failed selection still depends on initialization and zone-transition lifecycle.
That lifecycle is not yet reproduced or verified.

## Installed metadata inventory

Source files are under the original client's `Resources/sky`. SHA-256:

- `sky.ini`: `f68a926db340a53e2acb8b84f9f73ecda3b22f1555d18a347f81d52bbfa6842b`
- `weather.ini`: `344182753ca75cdc93832983bad0190b82443ae2778ce66f8fa4d468c582cbde`

A case-insensitive section/key scan finds 163 distinct `SkySetting-*` sections.
155 have a resolvable default pattern or no explicit default; eight reference
one of these missing patterns:

| Missing pattern | Authored settings |
| --- | --- |
| `clz-0` | broodlands, stillmoonb, thundercrest, arena, tutorialb |
| `NULL` | barter, bazaar, degmar |

Neither `clz-0` nor `NULL` is listed in `[WeatherPatterns]` or has a
`[WeatherPattern-*]` definition. `NULL` must not be classified as an intentional
sky-disable directive solely from its spelling. The native path below treats
an unresolved nonempty name as a failed lookup, with no special-case string
branch in the traced setter.

`ColorSet-BrightOverCast` selects the existing clz dawn/day/dusk/night maps.
This establishes a shared naming family, not an alias from `clz-0` to
`BrightOverCast`. No alias was added and no original asset was modified.
The inventory does not establish every listed setting's live accessibility,
server sky flags, cloud population, or complete texture availability.

## Native selection path

Evidence is the same installed 32-bit `EQGraphicsDX9.dll` as `SKY_COLORMAPS.md`:
SHA-256 `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below are virtual addresses, image base `0x10000000`; local annotated
inspection used `/tmp/eqgraphics-disassembly.txt`.

1. Zone setting selection at `0x100c2f20` looks up the zone's setting, then the
   literal `default` (`0x10135854`) if the zone setting is absent, storing the
   selected setting at sky-object offset `+0x34`. It calls virtual slot `+0x50`
   at `0x100c2fc9..0x100c2fd0` after selection.
2. Concrete sky vtable `0x101374dc` is installed at `0x1002d63c` and
   `0x1002d87e`. Its slot `+0x50` (`0x1013752c`) points to `0x1002c120`.
   That method reads the selected setting, falling back to object `+0x38` if
   needed, and reads its default-weather string at setting `+0x40`.
3. At `0x1002c194..0x1002c1a9`, it passes that string and transition time zero
   directly to weather-manager method `0x10032d20`. The manager is sky `+0x21c`.
   The call result is not used to choose another pattern in this method.
4. `0x10032d20` returns success if the requested name already matches the active
   pattern. Otherwise it rejects null/empty names at `0x10032db8..0x10032dc4`,
   calls the pattern registry lookup `0x10034490`, and rejects a missing registry
   node or missing payload at `0x10032dd2..0x10032dde`.
5. Those failures branch to `0x10032fb9`, return `AL=0` at `0x10032fc0`, and
   perform no weather-state write. Updates to previous/current pattern fields
   `+0x38c`/`+0x388` occur only later at `0x10032e8f..0x10032e9b` after lookup
   succeeds. The registry lookup returns null on exhausted buckets; it does not
   retry `DefaultClear`, a color set, or a renamed pattern.
6. The separate public weather setter at `0x1002c0d0` strips leading spaces and
   forwards to the same manager method. It contains no fallback either.

A related getter at `0x100c2e50` returns literal `DefaultClear` only when the
sky's selected *setting pointer* is null. Otherwise it returns the setting's
stored name at `+0x40`. This is not evidence for substituting DefaultClear when
an existing setting names an absent weather pattern.

The weather-manager constructor `0x10033990` initializes both pattern pointers
`+0x388` and `+0x38c` to zero at `0x10033aae..0x10033ab4`. Its reset method
`0x10033cd0` also clears those pointers at `0x10033d5f..0x10033d65`. Whether a
specific zone switch reuses an existing pattern, resets the manager, or installs
another pattern first requires tracing the actual host setup/transition calls.
A fresh construction and a retained manager are different cases. The graphics
reload path `0x1002d390` tears down prior sky data (`0x1002d220`), allocates a
new manager (`0x1002d4af..0x1002d4e6`), and calls its load/reset routine
`0x10033d70` at `0x1002d4ec`. That routine calls reset at `0x10033dd4` before
loading registries. This proves reload clears weather; it does not prove which
host transitions invoke reload. `0x1002c1c0` is the settings-save path, not a
zone-reset function; do not confuse its nearby addresses with reload.

## Next evidence required

Trace sky construction/reset and weather application from `eqgame.exe` during
zone entry, including server sky selection and any later explicit weather
request. Then compare a controlled native transition involving a missing
pattern, or verify the complete relevant call path. Until then, do not claim
that the current green fallback, DefaultClear, BrightOverCast, no dome, or the
previous zone's sky matches native behavior.
