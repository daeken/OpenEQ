# Native EQG material parameter binding

2026-10-01. The original client sends The Nest's authored `e_fSlide1X/Y`
and `e_fSlide2X/Y` values to `RegionLava` and `RegionWaterFall` unchanged.
The complete material reader and generic property binder execute in this
witness. A separate connected draw slice establishes that these writes occur
before the effect pass and indexed draw, including when the effect is already
active. This is research only; it does not change production rendering.

This closes the property-ingestion/upload gap in
[EQG_LAVA_WATERFALL.md](EQG_LAVA_WATERFALL.md). It complements the type-tag
evidence in [EQG_MATERIAL_PROPERTIES.md](EQG_MATERIAL_PROPERTIES.md) and the
independent clock witness in [EQG_EFFECT_CLOCK.md](EQG_EFFECT_CLOCK.md).

## Source and execution boundary

The graphics DLL SHA-256 is
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses use its preferred base `0x10000000`.

The probe independently inflates `thenest.eqg/ter_abyss01.ter`, verifies SHA-256
`238b0f41bd5133de9e9a5f04cc2502f090b120028bf7473b4f247340e3c2a176`,
and copies the exact original string table and selected material/property
records into controlled memory. It chooses first exact-name records 53
(`lavaflow`) and 3788 (`waterfalls`), consistent with the separately proven
material identity rule. It does not use OpenEQ's loader or reconstruct source
float words from decimal strings.

For each record, original reader `0x10014450` executes through its return at
`0x10015f19`. This includes name classification, material-name processing,
property ingestion, runtime type remapping and texture-frame bookkeeping.
Controlled interfaces supply allocation and narrow CRT functions. Archive
lookups return absent for the four requested `.txt` animation sidecars, after
checking that the names are absent from the original archive. Texture
acquisition records the four original texture names and returns null. Actual
texture loading, decoding, animation and texture-resource residency are outside
this witness. The original generic binder consequently issues null texture
bindings alongside the verified float bindings.

Effect endpoints are controlled, with parameter names/types/defaults parsed
independently from the installed compiled effects. The probe executes the
client's original calls into those endpoints, **not the D3DX implementation**.
It does not prove D3DX lookup case rules, technique validation, preshader
execution, vertex upload or GPU pixels. The draw slice supplies an active
single-pass effect; it establishes ordering for that active path, not which
technique an actual device admits. No original binary assets are committed.

## Property ingestion

Material fields `+0x4c` and `+0x50` hold the property count and a contiguous
12-byte runtime property array. Each entry contains a name pointer, runtime
kind and value word. At `0x10015370`, the native loop resolves the original
property name and dispatches the on-disk tag through `0x10015f1c`.

Ordinary tag-0 floats receive runtime kind 1. At
`0x10015431..0x1001543c`, the reader loads the source float with x87 and stores
it into the runtime value. All eight actual slide values retain their source
float32 words. This does not assert bit preservation for every possible IEEE
payload, such as signaling NaNs.

Two other names take special reader paths:

- Exact `e_fRenderType` becomes runtime kind 4 at `0x100153d2`; the ordinary
  float-value store is skipped. Its value field is not meaningful evidence
  from the probe's zero-initialized allocation.
- Exact `e_fEnvMapStrength0` becomes kind 1 but doubles the source float at
  `0x1001541c..0x10015429`. A controlled `0.25` source becomes `0.5`.

Neither exception matches a slide property. Disk tag 1 becomes runtime kind 2
with its word retained; the generic effect binder below does not upload it.
This is consistent with texture-channel metadata being distinct from float
effect inputs.

## Name lookup and float writes

Generic material binder `0x1008a110` obtains the material from the material
group's `+0x30` field, then walks its runtime property array in source order.
For kind 1, `0x1008a1f8..0x1008a217`:

1. Passes the retained name unchanged to effect vtable slot `+0x24`,
   `GetParameterByName(effect, null, name)`.
2. Skips the ordinary write if the returned handle is null.
3. Otherwise loads the runtime float, stores its argument as float32 and calls
   effect slot `+0x78`, `SetFloat(effect, handle, value)`.

The binder also checks for `e_fShininess` and can write a scaled
`a_fShininessLookup` parameter. That separate static branch does not match any
slide key; its derived write is not part of the executed slide cases.
Kinds 2 and 4 skip this generic upload. Kind 0 follows texture handling and
kind 3 follows packed-color conversion, outside the float proof.

Both compiled effects declare the four slide names as scalar floats. Their
defaults are float32 `0.02` (`0x3ca3d70a`) for slide 1 and float32 `0.03`
(`0x3cf5c28f`) for slide 2. The observed `SetFloat` arguments are:

| Material | Property | Source and uploaded word | Decimal description |
| --- | --- | --- | ---: |
| `lavaflow` | `e_fSlide1X` | `0x3e99999a` | 0.3 |
| `lavaflow` | `e_fSlide1Y` | `0x00000000` | 0 |
| `lavaflow` | `e_fSlide2X` | `0x3e4ccccd` | 0.2 |
| `lavaflow` | `e_fSlide2Y` | `0x00000000` | 0 |
| `waterfalls` | `e_fSlide1X` | `0xbdf5c28f` | -0.12 |
| `waterfalls` | `e_fSlide1Y` | `0xbea3d70a` | -0.32 |
| `waterfalls` | `e_fSlide2X` | `0x00000000` | 0 |
| `waterfalls` | `e_fSlide2Y` | `0xbf000000` | -0.5 |

Five controlled sequential cases execute against each effect interface:
an unknown float name gets a null lookup and no write; one known slide updates
only itself; a later material containing another slide leaves the previous
slide untouched; repeated names are written in source order, so the later
write wins; and a kind-2 property carrying a known slide name receives no
lookup or write. These prove the client's requested operations. The observed
retained values are state maintained by the controlled effect endpoint, not
an executed D3DX state implementation.

There is no blanket restoration of compiled defaults in this binder. An
implementation should therefore not assume that an omitted material property
causes the client to request its default again. Both actual Nest records
provide all four slide properties, so that omission issue does not affect
their verified values.

## Timing in the active region draw path

The witness executes native TER descriptor dispatch
`0x1008fb7c..0x1008fd02` from the parsed runtime type, and complete descriptor
initialization `0x100826d0`. It resolves Lava descriptor/effect `0x0e/0x0e`
and WaterFall `0xbf/0x2d`, agreeing with the earlier binding study.

Starting at `0x1008c812`, the connected region material-group caller executes
through `0x1008cc69`, stopping before list cleanup. A synthetic ordinary
one-triangle batch supplies the geometry fields. Native descriptor, global,
material, point and matrix binders execute; cached GPU setters and effect/device
COM methods are controlled. With a newly active effect the observed order is:

1. Effect `Begin(flags=3)` at `0x1008ca55`.
2. Global binder `0x10089590` at `0x1008ca5d`, including the already-proven
   native Time calculation for a controlled cached engine timestamp.
3. Generic material binder `0x1008a110` at `0x1008ca96`, issuing all four
   authored slide writes.
4. Ordinary batch routine `0x1008a9f0` at `0x1008cb0b`, reaching `BeginPass(0)`
   at `0x1008ad32`, `DrawIndexedPrimitive` at `0x1008ad62`, and `EndPass`.

Repeating the same group with the same effect already active skips effect
`Begin` and the global bind but still performs all four material lookups and
float writes before the next pass. All four draw cases complete the bounded
slice: initial and already-active cases for each effect. This establishes
per-material-group binding in this path, not a new OS clock read or a
material-local animation origin.

## Frozen reproduction

Run `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-material-parameter-binding.py`. The probe contains assertions for
the source and DLL hashes, exact runtime/output words, ten behavior cases,
reader special cases, native descriptor identities and four draw-order cases.
It executes 13 complete material-reader calls and 18 generic property-binder
calls; all assertions pass. The JSON contains metadata and call observations,
not original file bytes or pixels.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-material-parameter-binding.py` | `602e16dc322985ccb59ca12fbde0e51077a3248f63a5e2dd8eca3c9736399407` |
| `/tmp/openeq-material-parameter-binding.json` | `c863fdd7b20d3b2590ff757f4422536ee5d374dbd4bde636ce46062f123c21aa` |
| `/tmp/openeq-material-parameter-binding.log` | `b73f503a204202e4a36cd750224e36772b48b6a692d61e5ceedc858ae2a677f2` |
| `/tmp/openeq-pfsinspect.py` | `64fedc715bae14a56b0c10970914a003eab6165d282b71052a935906fd1ee0cd` |

The shared emulator prefix is the text before `renderer=alloc` in
`/tmp/openeq-ter-light-binding.py`; the probe verifies that prefix's SHA-256
`801c7c490d8d319d964fe43c975912d52001cc3c0bcb59f4aa11db86a813aa21`.
Effect hashes remain those recorded in `EQG_LAVA_WATERFALL.md`.
