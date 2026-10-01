# Native host ambient transfer and vision floors

2026-10-01. Research only; no production lighting changes in this slice.
The native host emits two distinct ambient inputs. Its ordinary ambient color
is the sky color raised to a vision-dependent minimum. A separate special
ambient term uses that minimum and a smoothed character-associated scalar.
Multiplying the sky color by that scalar would not reproduce this routine.

## Executed boundary

`/tmp/openeq-host-ambient.py` executes the complete original host routine
`eqgame.exe:0x004b6b20` under Unicorn, x87 control word `0x037f` and MXCSR
`0x1f80`. It also executes original vision-color initialization `0x005523b0`,
the concrete sky ambient getter `0x1002c730`, its manager getter `0x10032b60`,
and the original graphics-engine packed-color setters. The host's final
notification `0x005584b0` and alternate-path fog call `0x004acb30` are recorded
and intercepted. The ordinary path's intervening host arithmetic executes.

The clock, character profile accessor `0x00868900`, effect lookup `0x0044aba0`,
and character-associated virtual scalar method are controlled interfaces.
The latter returns an explicit float through x87 ST0. The sky manager points
to an explicitly supplied color table, including four words from original
installed DDS files. This witness does not execute full character logic,
geometry queries, sky resource loading or time interpolation, a native game
process, or a graphics device. Native x87 execution here does not establish
bit parity with every original CPU.

The [sky-color research](SKY_LIGHT_COLOR_INPUTS.md) establishes the separate
table-sampling boundary. The [renderer binding research](EQG_TER_LIGHT_BINDING.md)
establishes how the resulting engine fields reach effect parameters.

## Input selection

Host `+0x2d04 == 0` returns without clock calls, ambient writes or smoothing.
Otherwise status helper `0x005e8620` returns the object at global `0x010aa0c0`.
A nonzero byte at that object `+9`, unless environment byte `0x00f63490` is
7 or 8, selects a shortcut: ordinary ambient becomes packed `0xffffffff`,
the ordinary vision/sky/smoothing path is skipped, and a fog helper is called.
The meanings of that status byte and environment enum are not established.
The shortcut can obtain the character-associated scalar to raise its fog RGB
against existing configuration values; it does not update saved smoothing.

In the normal path the character at global `0x010171fc` is adjusted through
its virtual-base table and passed to the profile accessor. A profile dword at
`+0x332c` unequal to 255, or a nonzero result from effect lookup
`(65, 1, 0, 1, 1)`, enables infravision. Profile dword `+0x3348` unequal to 255,
or effect lookup `(66, 1, 0, 1, 1)`, enables ultravision and explicitly clears
infravision. Thus ultravision takes precedence when both conditions hold.
The names of effects 65/66 are corroborated by EQEmu's `common/spdat.h`;
the meanings of the two profile fields are not inferred as innate/racial data.

Zone-type byte `0x01024724` selects the base RGB:

| Zone type | Base RGB source |
| --- | --- |
| 1, 2 or 5, with sky | Sky virtual slot `+0x28` packed RGB |
| 1, 2 or 5, without sky | Three legacy hourly color tables |
| 4 | `[f32(0.5); 3]` |
| Other values | `[f32(0.1); 3]` |

Concrete sky vtable `0x101374dc`, slot `+0x28`, reads manager
`[sky+0x21c]`, then table `[manager+0x13c4]`. It copies table word `+0x1fc`,
entry `(31,3)` of the 32×32 color map, into a process-global cached word and
returns its address. The host extracts RGB, ignores alpha, and multiplies by
the f32 reciprocal of 255 at `0x00abe0e0` (`0.003921568859368563`). Base
channels are capped at one before the minimum is applied. Missing manager or
table within an existing sky object has the getter's separately documented
cached-color behavior; it is different from the host's missing-sky branch.

The hourly fallback reads R/G/B tables at `0x00ca59b0`, `0x00ca5a10` and
`0x00ca5a70`, uses hour minus one, wraps the next hour from 24 to 1, and
interpolates with minute times the native f32 reciprocal of 60. The witness
executes and asserts its 12:30 result, packed `0xffe5e5e5`; it does not sweep
all hours or invalid clock values.

## Minimums, scalar smoothing and packed outputs

Original initializer `0x005523b0`, executed before each witness case, supplies
the vision globals used by the host:

| Mode | Minimum RGB before packing |
| --- | --- |
| Normal | `[f32(0.08); 3]` |
| Infravision | `[65,50,50] * native reciprocal of 255` |
| Ultravision | `[80,80,120] * native reciprocal of 255` |

Infravision globals are at `0x01024434/38/3c`; ultravision globals are at
`0x01024440/44/48`. The normal minimum at `0x00abf12c` is exactly
`0.07999999821186066` as f32.

Global player `0x01017224` supplies an object pointer at player `+0x10dc`.
If present, virtual slot `+0xf4` returns target scalar `T`; otherwise `T=1`.
Saved scalar `S` is process-global `0x00c19b84`, initialized to -1. The sentinel
snaps immediately to the target. Otherwise each invocation moves by native
`f32(0.01)` toward the target and clamps overshoot to the target. This is a
step **per invocation**, not established as a fixed number of steps per second
or one step per rendered frame. Multiple host call sites reach the routine.

For ordinary finite scalar inputs exercised in `[0,1]`, the core transfer is:

```text
ordinary RGB = max(base RGB, minimum RGB)     # per channel
special A RGB = minimum RGB * (1 - S)
```

Both are packed with alpha FF and RGB bytes obtained by truncating channel
times 255, retaining the low byte. Saved smoothing and explicit local float
stores round to f32, while unspilled x87 arithmetic can retain extra precision
for the subsequent product. A model that rounds every intermediate to f32 is
not justified by this instruction trace. The independent bounded oracle
preserves source f32 constants/stores and wider unspilled arithmetic.

Engine global `0x01822678` has vtable `0x1013becc`:

| Slot / original setter | Packed field | Normalized RGB field | Host use |
| --- | --- | --- | --- |
| `+0xc8 / 0x1006b130` | `+0x234` | `+0x240` | Ordinary ambient |
| `+0xcc / 0x1006b1e0` | `+0x238` | `+0x24c` | Special ambient A |
| `+0xd0 / 0x1006b240` | `+0x23c` | `+0x258` | Not written here |

The host writes special A first, then ordinary ambient. The engine setters
decode the quantized RGB bytes using their own f32 reciprocal at `0x10135024`.
Consequently normal black produces packed `ff141414`, and the renderer gets
approximately `0.0784313753` per channel, rather than the host's prepacked
minimum 0.08. This is not an sRGB conversion or alpha multiplication.
Renderer assembly passes ordinary `+0x240` to `Ambient` and combines special
A `+0x24c` with special B `+0x258`, capped above at one, for `SpecialAmbient`.
The producer of special B remains a separate research boundary.

The host stores the mean of the raised ordinary RGB, before byte quantization,
at `this+0xc`. Its nominal incoming stack float argument is overwritten as
temporary storage; supplied arguments 0, 0.6 and 1 yield the same outputs in
the witness. Subsequent fog-color arithmetic executes, but this slice makes
no zone-fog parity claim for the controlled fog inputs.

## Witness results

For explicit sky word `ff4080c0`, ordinary ambient remains `ff4080c0` when
the scalar is 0, 0.25 or 1. Special A is respectively `ff141414`, `ff0f0f0f`
and `ff000000`. Starting from saved 0.25 with target 0.75 stores
`0.25999999046325684`; starting from 0.75 with target 0.25 stores
`0.7400000095367432`. Five successive upward steps, equality, absent-object
target 1 and clamping on both sides are also checked.

Black sky with normal vision produces `ff141414`; infravision produces
`ff413232`; ultravision produces `ff505078`. Both profile-field and effect
lookup routes are exercised independently, including ultravision precedence.
Zone type 0 produces `ff191919`; type 4 produces `ff7f7f7f`.

The following original installed DDS words pass through the actual sky getter,
host routine and engine setters with normal vision and default scalar 1:

| Source map | Incoming ambient word | Engine ordinary ambient word |
| --- | --- | --- |
| DefaultDay | `ffdbd9d9` | `ffdbd9d9` |
| DefaultNight | `ff000000` | `ff141414` |
| DefaultDawn | `00412838` | `ff412838` |
| DefaultDusk | `005c3e6b` | `ff5c3e6b` |

Source paths and exact DDS hashes are recorded in the JSON. There are 42
host executions: 39 checked by the bounded ordinary-transfer oracle, plus
explicit assertions for inactive, alternate and absent-sky behavior. Native
entry counts are 42 for host and vision initializer, 37 each for sky/manager
getters, 41 for ordinary setter and 40 for special-A setter. All assertions
pass; special B remains untouched in the ordinary cases.

## Provenance and remaining work

Original installed binaries:

- `eqgame.exe`: `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`
- `EQGraphicsDX9.dll`: `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`

Run `PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-host-ambient.py`.
Local artifacts, not committed with the original binaries/disassembly:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-host-ambient.py` | `517716fdebadd2225eeea6e0b3a097bcc01b39b1f18857dd441112f209ce7669` |
| `/tmp/openeq-host-ambient.json` | `3662580390d5c427739dc9badeb8121d4bc9d5a19d7521ba4d0a21725db4c24d` |
| `/tmp/openeq-env-game-frame.txt` | `8461765eafeca1d868b38430cabcdfae641a0ffc12d54e86c032d31bae901379` |

The harness reuses only the dual-image setup prefix of
`/tmp/openeq-host-sky-light.py`, SHA-256
`56871adeb13c9d81fb9364c8aa9ad8312f1218917528f68eceace7ded8d144e0`,
which in turn reuses the helper prefix of `/tmp/openeq-ter-light-binding.py`,
SHA-256 `9c278977e3c7d9c8b983489f34f7aef8a3a49c51dea5a5b6ddcd816a787e1fb6`.
Those prefixes map the original images and provide ordinary memory interfaces;
their unrelated render/host experiments do not execute here.

The character-associated scalar's producer is not yet proven. Static review
finds graphics-actor vtable `0x101390fc`, slot `+0xf4`, pointing at
`0x1003a2a0`, a candidate involving timed geometry queries and interpolation.
This slice does not establish that the host's player `+0x10dc` object has that
vtable and does not execute the candidate. It must not be renamed an outdoor,
occlusion or brightness factor on this evidence alone. The static candidate
disassembly `/tmp/openeq-host-ambient-actor-scalar.txt` has SHA-256
`cbbda23f03580618aea4672abba0366ce64f31460989753a8b9047b6cf54a812`.
Live character conditions, the scalar's geometry path, special-B provenance,
update scheduling and complete fog behavior remain necessary before claiming
full environment-lighting parity.
