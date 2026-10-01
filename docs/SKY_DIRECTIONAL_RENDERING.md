# Authored sun and moon color in surface lighting

October 1, 2026. The renderer now selects the original sky table's directional
RGB for native environment/time types1/2/5. This replaces the fixed warm-white
color for admitted skies. Direction, shadow projection, bounce, special vision
and final display color remain existing OpenEQ policy; this is not full native
lighting parity.

## Input and lifetime

`SkyColorMapProvenance` retains `day_fraction_bits`, the exact normalized f32
input already used to compute the table's truncated day tick. Keeping this
with the sampled table avoids using a newer live minute with an older background
sky result. Main, offline and diagnostic renderers inherit the same data through
`load_sky`; no duplicate clock or extra refresh job is introduced. Existing
normalization remains unchanged: finite inputs use f32 rem_euclid(1), nonfinite
inputs use0.5, and tiny negative inputs may round to1.

For admitted types, a valid32×32 OriginalDome table and finite provenance
fraction in[0,1] select sun inside inclusive float-bit boundaries
`0x3e7c71c7..=0x3f438e39`; outside selects moon. The table swatches are column31,
row0 for sun and row1 for moon. Adjacent floats can have the same truncated tick
while selecting different lights, so reconstructing the fraction from the tick
would be incorrect. Original host clock arithmetic varies with x87 rounding
at18:20; the integration uses OpenEQ's exact sampled fraction and makes no
universal native raw-hour cutoff claim.

RGB bytes are normalized independently, ignoring alpha and without the ambient
floor. Existing Rust division and linear-light shader arithmetic remain; native
reciprocal multiplication and encoded display transfer are separate. Current
light direction remains fixed. No bounce approximation, new GPU pass, new buffer
layout or allocation is introduced.

Every `set_environment` recomputes directional color. Missing tables, missing
provenance, malformed inputs and unsupported types restore the prior warm-white
fallback `[1,0.96,0.86]`; this explicit OpenEQ policy does not reproduce native
cached-object/definition update lifecycles. Camera-liquid presentation leaves
selection intact and restores the same above-water lighting on exit.

## Evidence and verification

The connected original host→selector→table getter→packed setter witness is in
`NATIVE_SKY_DIRECTIONAL_COLOR_SELECTION.md`. Root replay matches frozen JSON
SHA-256`3f19a31fb68036e0224b09334d05101924049280c6a05420daf4e3b5064359eb`.
Its18,007 native executions include every valid minute under12 FPU modes,
all256 zone types at day/night, threshold neighbors, alpha controls and original
DDS inputs. This is original instruction evidence, not native framebuffer proof.

The new provenance regression exercises exact threshold neighbors, finite
normalization, nonfinite fallback and tiny negatives. Existing original table
fixtures continue matching1,131 native samples. Directional tests cover all256
zone types, both threshold equalities/neighbors, alpha/floor independence,
malformed/missing fallback, GPU day/night selection and reset, liquid exit,
emissive/empty-sky isolation, and original PoK day/night surface input.

Full-scene review and final combined checks are recorded in the daytime log.
Original binaries, textures, derived captures and probes stay outside Git.
