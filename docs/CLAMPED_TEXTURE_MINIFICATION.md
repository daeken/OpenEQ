# Minification of opaque clamped textures

October 1, 2026. Opaque clamped atlas materials now use the existing mip chain
with real clamp-to-edge sampling. Previously their shader always sampled level
zero, so distant baked terrain could shimmer even though repeating opaque
materials already selected mips from their pixel footprint.

The repeat sampler and manual half-texel inset were sufficient only at level
zero. Reusing that inset at a coarser or fractional mip would sample the tile's
opposite edge. Atlas binding8 now supplies a separate trilinear clamp sampler;
the geometry pass uses original interpolated UVs and explicit derivatives.
There is one additional sampler per atlas bind group, with no added texture,
vertex data, render pass or per-frame upload.

This is an OpenEQ filtering correction, not recovered original device policy.
The existing byte-space CPU mip construction, atlas rescaling and sRGB handling
remain unchanged. Alpha-tested, blended, additive, water and waterfall sampling
retain their existing policies. In particular, foliage/shadow coverage still
uses level zero and needs a separate coverage-preserving mip design. Direct
terrain paint sampling is separate; this change reaches opaque baked/fallback
materials carrying the existing clamp flag.

## GPU evidence

Two new GPU regressions establish both minification and addressing:

- Subpixel one-texel stripes sampled inside a clamped tile produce uniform128
  with zero measured camera-motion change at three tested footprints. A fully
  opaque alpha-test control uses the retained level-zero path: range226 and
  mean motion83.625/85.078/88.078 byte levels. Close-up four-texel stripes retain
  their black and white interiors. These values describe the fixtures, not a
  guarantee for arbitrary content or near-Nyquist detail.
- Distinct opposite edges remain identical to a solid-edge reference across
  both axes, both ends, out-of-range UVs and two coarse/fractional mip footprints.
  The same inputs with repeat addressing deliberately differ. Existing opaque
  and blended level-zero edge regressions also pass.

The original Feerrott2 baked-terrain GPU fixture passes with329 clamped material
records, unchanged vertex/index buffer sizes and scene bounds. Its clamp versus
repeat comparison changes3,926/518,400 pixels; this comparison isolates edge
addressing under the new filtering, not an original-client image comparison.
No source terrain or collision data changes.

Focused logs: `/tmp/openeq-clamped-mip-controls.log` and the terrain-addressing
portion of `/tmp/openeq-clamped-mip-tests.log`. The latter also contains an
initial rejected minification fixture whose lowest sampled mip still resolved
some stripes; the final test moves both selected mips beyond that frequency.
The original first test draft also had a private-field compile error, corrected
before GPU results were accepted. Final renderer-suite/build/lint outcomes are
recorded in `DAYTIME_2026-10-01.md`.
