# Native waterfall preshader precision

2026-10-01. Original Microsoft D3DX9_30 instructions confirm that the compiled
`RegionWaterFall` preshader stores its intermediate values as **doubles** and
converts its shader constants to float32 afterward. For the tested domain,
compute the preshader phase in float64, multiply by the authored float32 rate
expanded to float64, then round the offset to float32. A single float32
`time * rate` is not equivalent for all observed waterfall materials.

This supplies the numeric execution evidence missing from the static decode in
[EQG_LAVA_WATERFALL.md](EQG_LAVA_WATERFALL.md). The independent clock and authored
property bindings are documented in [EQG_EFFECT_CLOCK.md](EQG_EFFECT_CLOCK.md)
and [EQG_MATERIAL_PARAMETER_BINDING.md](EQG_MATERIAL_PARAMETER_BINDING.md).
This note changes no production code.

## Original instructions and scope

The installed EQ graphics DLL imports `d3dx9_30.dll`. The local original
Microsoft D3DX30 binary at `/tmp/openeq-d3dx9-native/d3dx9_30.dll` has SHA-256
`5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532`.
Addresses below use that binary's preferred base `0x00400000`.

The witness reads `RegionWaterFall.fxo`, SHA-256
`65aa785ab770b621669e65f7e5b99e2397e8eeaac7efc4d398019cdf916514fd`.
Its two `PRES` blocks at file offsets `0x130c` and `0x2490` contain identical
1,768-byte preshader programs, each SHA-256
`35420a0ed03c3a911f03179f0fa2f553403657a47765899ffed22cecc184ea20`.
Both original programs are parsed and executed independently.

Executed native functions:

- `0x004eac2d` / `0x004eab4e` validate the original preshader and return its
  original `FXLC` instruction and `CLIT` constant tables.
- Runtime caller `0x005053ee` retrieves a controlled `ID3DXBuffer` layout and
  invokes interpreter `0x004ea21f` at `0x00505541`.
- The interpreter resolves temporary register `rN` to an eight-byte slot
  (`0x004ea33a..0x004ea346`, `0x004ea3d9..0x004ea3e5`). Scalar multiplication
  loads two qwords and stores a qword at `0x004ea5e6..0x004ea5f1`.
- Negation, maximum, comparison, addition and fractional-part instructions also
  execute original code. The fractional operation calls CRT `floor` and
  subtracts that result, storing a qword at `0x004ea6f6..0x004ea706`.
- After interpretation, the original caller converts the output bank from
  qword values to float32 with `fld qword` / `fstp dword` at
  `0x0050557e..0x0050559e`.

The parameter bank is supplied as exact float64 expansions of controlled
float32 effect values, using the register positions from the compiled constant
table. This does **not** execute D3DX's parameter-bank population or a complete
effect constructor. The buffer pointer/layout interface and CRT `floor` are
controlled; `floor` returns the mathematical result through an x87 return stub.
No preshader opcode, arithmetic intermediate or final output conversion is
replaced. The x87 control word is explicitly `0x037f` and MXCSR is `0x1f80`.
No graphics device, technique validation, vertex shader or GPU pixels execute.

## Calculation and distinguishing cases

The compiled program evaluates the following operations with a double store
between its arithmetic instructions:

```text
t = float64(effect_time_float32)
x = t * float64(0.01)
phase = copysign(fract(abs(x)), x) * float64(100.0)
offset = float32(phase * float64(authored_rate_float32))
```

The displayed formula summarizes the original negate/max/compare/add/multiply
sequence. Its positive finite clock domain is `0 <= effect_time < 100`.
Negative and beyond-period inputs are also sampled below; exceptional IEEE
inputs and every possible x87 precision setting are not claimed.

All 100,000 possible integer-millisecond remainders of the independently proven
clock are run through the original interpreter and output conversion using
The Nest's rates `(-0.12,-0.32,0,-0.5)`. All 400,000 output words match the
double-stage formula. They also happen to match a direct time/rate multiply for
**these four rates**. That coincidence does not extend to all authored rates.

At native clock remainder **237 ms**, with its exact float32 clock conversion,
the original instructions produce:

| Authored rate | Native / double-stage output word | Direct time × rate output word |
| ---: | --- | --- |
| 0.75 | `0x3e360419` | `0x3e36041a` |
| 3.0 | `0x3f360419` | `0x3f36041a` |
| -1.5 | `0xbeb60419` | `0xbeb6041a` |

A different shortcut, performing the phase computation with float32 temporary
values, already differs at 17 ms for The Nest's rates. Both shortcuts discard
observable rounding behavior.

The additional corpus witness reads the frozen waterfall inventory containing
57 distinct authored rates. It executes 390 batches: all rates grouped in
fours, 13 selected millisecond remainders, and both original preshader programs.
The selected remainders are `0, 1, 17, 237, 999, 1234, 12345, 33333, 50000,
66667, 90001, 99998, 99999`. All native results match the double-stage formula;
six batches differ from direct multiplication. This is representative native
coverage of all 57 rates, **not** an exhaustive 57-rate native clock sweep.

Separately, 84 cases cover both programs with Nest, compiled-default and lava
rate sets at positive, negative, near-period and beyond-period times. All match
the double-stage calculation. The broader implementation's CPU/GPU testing is
separate from this original-instruction witness.

## Frozen reproduction

Both probes use `PYTHONPATH=/tmp/openeq-re-tools`. The main probe includes the
100,000-step native clock sweep; the corpus supplement runs the 84 base cases
and its 390 batches without repeating that sweep. JSON outputs contain derived
numeric data only. The 400,000 Nest output words, packed little-endian in time
and slide order, have SHA-256
`8208b73dd2d5877d67cdfb3518074fe93f85cefbcc5d71b98f5d23ce6b911db7`.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-waterfall-native-preshader.py` | `945c13d5e4ee0f6731beb3de2dd92279bf32d35ff50896e2787578c59b583d31` |
| `/tmp/openeq-waterfall-native-preshader.json` | `e49d0b5e539838526455e147ee289bf66803e4446ea8831c6a847c11ad5e0686` |
| `/tmp/openeq-waterfall-native-preshader.log` | `dbfac5641df09c41279f9d8635be3185b0b232d1dc0008d1582035bb572f6283` |
| `/tmp/openeq-waterfall-corpus-preshader-supplement.py` | `48524efe47176741dab28111fbdbb67185c36fbc56e51ab4d184bdab1488fed8` |
| `/tmp/openeq-waterfall-corpus-preshader-supplement.json` | `b0982df7c5377c93ca87926951454c4aa485723aebdc173862bc076f3334e861` |
| `/tmp/openeq-waterfall-corpus-preshader-supplement.log` | `6f04f8b0df915b188d710fdbef9e49c544b2811a03b00cb5d77aafdb74746e08` |
| `/tmp/openeq-waterfall-corpus.json` | `f263478b03c37e79e5dac5d3b131ab69f10fee918ff928135490650d76210718` |

The main probe verifies its shared D3DX emulator prefix (the text before
`locale=alloc` in `/tmp/openeq-particle-d3dx-textures.py`) against SHA-256
`2fddd10c457b671e99d0758b96c6ddabded335940dbcd94b8ab788870156ffd3`.
The supplement verifies the main probe's pre-sweep prefix and the corpus hash.
