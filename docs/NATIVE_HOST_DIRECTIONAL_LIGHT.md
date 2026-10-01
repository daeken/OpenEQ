# Native host sky-to-light transfer

2026-10-01. Research only; no live environment shading changes. This connects
`eqgame.exe`'s directional/bounce update to the graphics-engine inputs already
established in `EQG_TER_LIGHT_BINDING.md`. Sky color-map getters and their time
sampling are investigated separately in `SKY_LIGHT_COLOR_INPUTS.md`.

## Executed scope

The complete host routine at `0x004b9440` executes with explicit zone type,
active flag, clock, sky-interface colors and angles. Original graphics-engine
setters, directional-object methods and color-definition methods execute.
The trailing call to the separate ambient routine is recorded and intercepted;
character vision and ambient smoothing are not included in this witness.
Allocation, free and memcpy are ordinary controlled memory interfaces.

Two probes distinguish host arithmetic from trigonometry. The first supplies
asymmetric math-interface outputs (0.25, 0.75). The second constructs the original
512-entry trigonometric tables at `0x100ba060`, uses original vtable `0x10134fb0`
and executes the actual interpolated sine/cosine methods `0x100b9cc0` and
`0x100b9d20`, including their float-to-integer helper. These tables use the
client's 512-unit angular convention, not radians at the interface.
The probe's sky angles are still explicit inputs, not recovered live values.
Unicorn x87 execution does not establish bit parity with every original CPU.

The global graphics engine at host `0x01822678` comes from
CreateGraphicsEngine output +8 (`0x009336ca..0x009336cd`). Its math interface at
`0x01822684` comes from output +0x18 (`0x009336e5..0x009336e8`), which the DLL
populates from `0x1017c0b8` at `0x100124ae..0x100124b5`. Construction installs the
math vtable at `0x100123db`, then calls table constructors including `0x100ba060`.

## Conditional host behavior

Host +0x2d04 gates the entire update. If zero, none of the witnessed clock, sky,
light, bounce or ambient calls occurs. Zone-type byte `0x01024724` admits values
1, 2 and 5 to the sky path. Other values set the host's light **definition** to
black, immediately clear engine bounce and disable the sky interface if present.
They do not immediately clear the directional object's cached RGB in this
routine; subsequent definition-to-object refresh is a separate lifecycle step.

For an admitted zone with a sky interface:

1. Clock helper `0x004add30` supplies hour/minute. The routine stores
   `(hour * 60 + minute) * f32(1/1440)` as a float and calls sky slot +0x48.
   For 12:30 the passed fraction is 0.5208333730697632.
2. Sky slot +0x4c chooses the moon branch when nonzero. Sun uses angle slot
   +0x18 and color +0x30; moon uses angle +0x20 and color +0x34.
3. The host evaluates the math interface at `512 - angle`, writes direction
   `[0, sine, -cosine]` through directional slot +8, then passes the packed
   color pointer to directional slot +0x0c. No normalization follows here.
4. Sun bounce comes from sky +0x3c, moon bounce from +0x40. Its packed pointer
   passes to engine slot +0xd4 (`0x1006b190`).
5. The separate ambient routine `0x004b6b20` is called with a scalar selected
   from hour-indexed table `0x00ca58c0`, using host byte +0x150.

If an admitted zone has no sky interface, the host writes a grayscale value from
that hour-indexed table into the light definition, then calls ambient. It does
not immediately rewrite the directional vector, cached object RGB or bounce.
This distinction rules out inventing a universal reset-to-black policy from
this routine alone.

## Color and direction witnesses

Both the native directional packed-color setter and bounce setter take source
bytes 2, 1, 0 as RGB and multiply by the stored f32 reciprocal 255 at
`0x10135024`. Source byte 3 is ignored. This is neither an sRGB decode nor an
alpha multiplier. For sun packed word `0xa0123456`, RGB becomes approximately
`[0.07058824, 0.20392159, 0.33725491]`. Moon `0xb0654321` gives
`[0.39607847, 0.26274511, 0.12941177]`. Distinct bounce words verify the separate
sun/moon getter selection. All seven conditional cases record the expected effects; the absent-sky
case is recorded without a separate assertion. Independent review reran the
native-math witness and reproduced its result hash exactly.

With explicit sun angle 0.5 and moon angle 1.25, actual original math produces:

| Branch | Direction written to the native light |
| --- | --- |
| Sun | `[0, -0.006135769188404083, -0.999962329864502]` |
| Moon | `[0, -0.015338961035013199, -0.9998682737350464]` |

The control math interface instead yields `[0, 0.25, -0.75]`, confirming the
host's axis/sign convention independently of the table implementation. Angle
writers and actual update ordering remain unproven; the sky time setter alone
does not write the fields returned by its angle getters.

## Provenance and limitations

Original hashes:

- `eqgame.exe`: `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`
- `EQGraphicsDX9.dll`: `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`

Local probes (original binaries and generated disassembly are not committed):

- `/tmp/openeq-host-sky-light.py`:
  `56871adeb13c9d81fb9364c8aa9ad8312f1218917528f68eceace7ded8d144e0`
- Its JSON: `f3e0eb4e805dbfc6e00107aae631d7da8412a5544651d5c3cb9d311492a01e4e`
- `/tmp/openeq-host-sky-light-native-math.py`:
  `f524ec198438d0cc1e1c93019d7105809c1b3056c771a8fe8fada307a8d186db`
- Its JSON: `a2f273db6519835b3e566a84e4394863554eeeb8019515bdeb6940071489d935`

Both probes reuse the initialization/helper prefix of
`/tmp/openeq-ter-light-binding.py`, whose source hash is recorded in
`EQG_TER_LIGHT_BINDING.md`. No game process, network session or native GPU is
started. These are bounded original-instruction tests, not original rendered
frame comparisons. Host ambient, directional-angle provenance, definition
refresh timing, full sky lifecycle, DPVS light membership and complete native
shader/device selection remain necessary for environment-lighting parity.

## Follow-up: executed host angle writers

The follow-up `/tmp/openeq-host-sky-angle-state.py` executes the complete host
initialization `0x004b96d0`, which ends by tail-calling `0x004b9440`. The sky
interface now uses original time/switch/angle getters and setters, while its
packed color getters and the external game clock remain controlled inputs.
It independently initializes every minute of a 24-hour day (1,440 cases) at a
fixed millisecond clock, erasing prior angle globals between cases. This proves
initialized state for those inputs, not that the live client reinitializes
its sky every minute.

At `0x004b98e8`, the original routine writes the sun angle through sky +0x1c
(`0x100c2cd0`). At `0x004b9912`, it writes the moon angle through sky +0x24
(`0x100c2cf0`): the original wrap helper `0x0092c450` receives sun angle +256.
The initial sun value is computed from separately established globals:

```text
sun = wrap512(float(unsigned(clock - origin) * delta / duration + base))
moon = wrap512(float(sun + 256))
```

This notation describes the data flow; the witness preserves original x87
instructions and stores rather than assuming arbitrary-language bit parity.
Initialization sets delta=128 and duration=1,080,000 milliseconds in these
cases. Its piecewise hour/minute logic chooses base/origin and an ambient-table
index. Noon initializes sun=0, moon=256; midnight gives sun=256, moon=0;
06:00 gives sun=384, moon=128; 18:00 gives sun=128, moon=384.
Independent reinitialization near dawn/dusk does not reduce to a single linear
minute-to-angle formula, so these observations do not authorize one.

The subsequent bounded update block `0x004b9a53..0x004b9ad5` is also executed
with its original incoming host/frame state. It reads the saved origin/base/
delta/duration and rewrites both angles without reinitializing them. Five
midday controls at elapsed 0, 1, 3,000, 45,000 and 1,080,000 milliseconds give
sun angles 0, 0.0001220703125, 0.35552978515625, 5.33331298828125 and 128.
Every minute case checks finite angles in [0,512) and the wrapped 256-unit
moon offset. The full native table outputs and bounded updates are retained.

This resolves the angle-writer identity; actual caller cadence, calendar/time
receipt, weather/dome/frame ordering and full initialization lifecycle remain
separate. The earlier direction probe's explicit angles remain valid controls.
No runtime angle, lighting or sky-geometry policy is changed by this note.

Follow-up hashes:

- Probe: `fcbcbc21c58539b468c179be997fe7476ceda755d88aa3004137ed0890489875`
- JSON: `f7ea17efaa2a6e873686c683c6cbf5f696e47f3c1d787668bb742d6892d09e96`

This probe uses the frozen native-math probe's initialization prefix. Original
binary hashes and process/device exclusions are unchanged.
