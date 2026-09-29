# Bounded indexed heightmap water surfaces

The bounded surface slice described here is implemented. Verification and
remaining limitations are recorded below. Native evidence and exact addresses are in
[HEIGHTMAP_WATER_REVERSE_ENGINEERING.md](HEIGHTMAP_WATER_REVERSE_ENGINEERING.md);
the original six-zone audit is in [HEIGHTMAP_WATER_PLAN.md](HEIGHTMAP_WATER_PLAN.md).
This slice adds visible, textured surfaces from authored records. It does not
create liquid volumes, change swimming/camera state, or interpret the unknown
trailing float as a bottom.

## Eligibility and diagnostics

A tile may produce one indexed surface only when:

1. Its modern water extension exists and its raw tag is nonzero. Signed tags
   `-128` and `-1` are active, just like `1`.
2. Its selector is in **1..9999**, the native rectangular-grid construction
   branch decoded so far. Metadata continues preserving all other selectors;
   zero, negative and >=10000 selectors do not gain guessed geometry.
3. `WaterData::resolve_index` returns `Unique`. Identical duplicate definitions
   are permitted by that result; missing or conflicting definitions are not
   silently replaced by index zero, the first definition or a synthesized one.
4. The elevation, bounds, derived tile width and translated positions are
   finite, and the rectangle passes the validation below.

Resolve against the actual `water.dat` associated with the loaded heightmap,
including an archive whose name differs from the internal terrain name. Keep
finite `*WATERSHEET` handling independent. An inactive tile does not become
water merely because its base elevation is above some terrain vertices.

Unsupported or malformed active records should produce a summarized diagnostic
with zone, tile coordinates, selector and reason. Omit those individual indexed
surfaces while retaining terrain and metadata. Do not generate magenta stand-in
planes, flood full tiles, or silently change bounds to make a record fit.

This is a deliberately narrower rendering policy than the original constructor's
null-material fallback. The metadata resolver and supported native branch give
the caller an explicit basis for each drawn surface.

## Coordinates, bounds and geometry

All values entering the asset mesh use the existing **EQ scene axes**:

```text
spacing = options.units_per_vertex
q = options.quads_per_tile
tile_width = spacing * q
origin = [tile.longitude * tile_width, tile.latitude * tile_width, 0]
bounds = [xmin, xmax, ymin, ymax]       // authored tile-local XY
surface_z = tile.water_level          // absolute scene elevation
```

The surface occupies `origin.xy + [xmin..xmax, ymin..ymax]` at `surface_z`.
Do not add sampled terrain height to Z. Do not apply EQEmu's X/Y swap in asset
loading. The renderer's existing scene-to-render transformation remains the
single coordinate conversion.

For the initial audited branch, require:

- `spacing > 0`, finite positive `tile_width`, and existing supported `q`.
- Every bound and `surface_z` is finite.
- `0 <= xmin < xmax <= tile_width` and `0 <= ymin < ymax <= tile_width`.
- Checked finite translated endpoints; multiplication of grid coordinates by a
  finite width can still overflow.

Reversed, zero-area, nonfinite or out-of-tile bounds are rejected for rendering,
without sorting, clamping or expanding them. These checks are **OpenEQ policy**
for a bounded implementation, not a claim that the native code validates them.
The metadata parser intentionally remains lossless, including unusual float bits.

Use the native subdivision rule for valid extents:

```text
nx = max(1, trunc((xmax-xmin) / spacing))
ny = max(1, trunc((ymax-ymin) / spacing))
x(i) = xmin + (i/nx) * (xmax-xmin), i = 0..nx
y(j) = ymin + (j/ny) * (ymax-ymin), j = 0..ny
```

Generate the full grid, keeping endpoints exactly at the authored bounds. The
native generator emits both sides: two sets of `(nx+1)*(ny+1)` vertices with
opposing Z normals and four triangles per cell. Match this arrangement or use a
renderer-equivalent two-sided arrangement only if back-face normal handling is
explicitly verified. Native rectangle geometry and the examined shaders do not
clip cells against terrain heights or quad flags. Ordinary terrain depth
occlusion covers the parts below solid ground.

Every generated surface must be excluded from collision geometry. Water draws
and collision must remain separately testable, including any new scene-side
collision collection. No invisible collision plane is required beneath it.

## UVs, boundaries and animation

Compute UVs per grid vertex **before interpolation**. For the audited tile-local
rectangles, the native positive remainder of the translated start is the local
start, so its grid lookup is:

```text
gx = trunc(x(i) / spacing + 0.5)
gy = trunc(y(j) / spacing + 0.5)
cpu_uv = f32([gx, gy]) / f32(q)
packed_uv = trunc(cpu_uv * 256)
asset_uv = packed_uv / 256
```

Check that the lookup indices are in `0..=q`. A non-grid-aligned but otherwise
valid rectangle follows the same nearest-grid lookup; do not read outside the
native lookup table. Preserve the float-to-integer truncation step. The native
GPU input is unnormalized SHORT2; storing the unpacked `asset_uv` as floats in
OpenEQ preserves the same intended coordinates without requiring a new vertex
format.

Keep `asset_uv=1` at a tile's maximum edge. Do not apply `fract` to mesh UVs:
interpolating from a near-one value to zero across the final cell stretches the
texture backwards. Repeat addressing handles sampling beyond one. Opposite
tile-edge values 1 and 0 agree under repeat at integer `UVScale`; their second
layer differs by two periods. This also works across negative world coordinates
because the computation is tile-local, not a signed world-coordinate remainder.

The observed active scales are:

| Zone | Active selector(s) | Authored UVScale |
| --- | --- | --- |
| Feerrott2 | 1 | 1.000 |
| Dead Hills | 1 | 1.000 |
| Loping Plains | 1, 2 | 1.000 |
| Buried Sea | 1, 2 | 1.000 |

Old Commonlands has 83 index-zero definitions with scale 3.100 but no active
indexed rectangles. Preserve arbitrary finite authored scale values, including
fractional values; do not round them to repair a seam. Noninteger scales do not
generally give equal samples at opposite tile edges. Reject a scale only if its
derived coordinates would become nonfinite; do not invent a semantic range.

Add an explicit optional indexed-water UV mode to the material/render parameters.
Default existing EQG and finite-sheet materials to the current path. For the new
mode, pass `asset_uv` through the vertex shader and use:

```text
phase = fmod(seconds, 100)
uv1 = asset_uv * authored_uv_scale - phase * [0.02, 0.02]
uv2 = asset_uv * authored_uv_scale * 2 + phase * [0.03, 0.03]
```

The native formula uses `a_fTime`; using OpenEQ elapsed seconds is an explicit
timing convention until the original time provider's unit and epoch are traced.
The default phase reset is periodic under repeat because the two scrolls reset
by exactly two and three texture periods. Keep rates independent of UVScale.

Use derivatives of the selected base UV for normal-texture sampling, scaling
them by two for the second layer. Compute the necessary derivatives before
material-dependent control flow as required by WGSL uniformity rules. The
existing `/80` world UVs and scale 1.37 must remain the default for older water
materials. New indexed parameters must not change those existing values.

The first slice can retain the current water color, reflection, Fresnel and
lighting approximation while supplying the authored material values and native
indexed normal-map coordinates. Explicitly retain that appearance limitation:
the original DX9 pixel shader has different normal/distance/lighting and alpha
math, and the surrounding native blend/depth states have not been fully traced.
No vertex waves, terrain-height sampler or extra shoreline clipping are needed
for the surface geometry established by this research.

## Resource bounds

Compute counts with checked arithmetic before allocating. For two-sided native
geometry, one rectangle needs `2*(nx+1)*(ny+1)` vertices and `12*nx*ny` indices.
The validated within-tile bounds imply `nx,ny <= q`; explicitly enforce that
relationship rather than relying on a float-to-integer cast to bound allocation.

A concrete proposed budget is **2,000,000 generated vertices** and **12,000,000
indices per zone**, counted across indexed water before allocation. This is an
OpenEQ allocation policy, not a decoded format limit. At the existing 32-byte
asset vertex representation it caps the new geometry near 112 MB before upload.
It comfortably covers Feerrott2 and even the full-tile upper bound for all 900
Buried Sea rectangles at `q=24`. Cache material/texture lookup by resolved
definition; do not reload shared maps for each rectangle.

If the total exceeds the budget, return a clear indexed-water build diagnostic
and retain the zone's terrain without a partly generated indexed-water set.
Keep generation transactional so an allocation failure cannot leave half-built
mesh/material references. Existing archive/image-size limits continue applying
to textures. Do not increase terrain allocation limits to make water fit.

## Acceptance checks

- Synthetic geometry: byte zero produces no sheet, selectors outside the
  decoded branch produce no guessed sheet, duplicate-identical lookup produces
  one sheet, and missing/conflicting lookup reports a reason. Zero/negative
  elevations remain valid; the unknown tail has no effect.
- Bounds and budget: reversed, degenerate, NaN/infinite and out-of-tile bounds
  are rejected for drawing while metadata survives; count overflow and total
  resource limits stop the indexed surface set before allocation.
- Geometry: all triangles stay inside the authored XY rectangle at the exact
  base elevation, have both intended orientations, and appear in no collision
  query. Terrain height changes do not expand or crop the surface.
- UVs: test `q=16` and a `q` that does not divide 256, endpoints 0/1, positive
  and negative tile coordinates, noninteger scale preservation, and derivatives
  for both layers. Verify a two-triangle simplification would not substitute
  for the native grid quantization.
- Existing behavior: one EQG water fixture and one finite heightmap sheet retain
  their default UV mode, material parameters, and collision behavior.

Feerrott2's original records give exact acceptance anchors:

| Fixture | Expected result |
| --- | --- |
| Full zone | 65 indexed rectangles; 8,533 grid cells; 34,132 triangles with native two-sided subdivision. |
| Tile 84 `(-3,-12)` | Bounds `[-768,-672,-2960,-2848]`, Z=-50; `nx=6`, `ny=7`; 112 two-sided vertices and 504 indices. |
| Tile 84 UV bounds | U=`0..0.375`, V=`0.4375..0.875`, before authored scale and scrolling. |
| Tile 11 `(1,2)` | No indexed surface despite base elevation -30; tag is zero. |
| Tile 271 `(-7,2)` | Native 16×16-unit rectangle remains even though all terrain is above it; ordinary depth occlusion decides visibility. |
| Tiles 109/84 seam | Matching Z=-50 and repeated UV phase at world X=-768, Y=-2944..-2864. |

Verify that all new materials/textures resolve without magenta fallbacks and
inspect fixed headless captures at the pond edge, seam, and tile 271. Buried
Sea selector 2 must use its own gray material values, while Old Commonlands must
gain no indexed surfaces. These checks validate the bounded surface slice;
matching the original client's complete appearance remains a later comparison.


## Implementation and verification

The pure asset baker produces native-style two-sided grids with opposing normals,
per-vertex fixed-256 quantization and unwrapped 0/1 tile endpoints. Twelve
synthetic tests cover actual extents/winding, q14/16/24 interpolation, tags and
selectors, malformed metadata, exact duplicate/conflicting materials, all bounds
and precision failures, finite scales, diagnostic caps and transactional budgets.
There are at most 128 detailed rejected-record diagnostics, plus an omitted count.

The loader shares materials by selector. Authored texture paths remain intact in
metadata; rendering resolves basenames only through existing archives/known client
texture directories. Unresolvable or undecodable maps omit that selector's
surfaces with a diagnostic. Invalid indexed metadata or an excessive whole-zone
budget preserves existing terrain and finite sheets. Three loader regressions
cover malformed metadata, budget failure and missing original-resolver maps.

An explicit optional material mode selects asset UVs in the GPU shader. Existing
EQG/finite-sheet materials retain the prior world-coordinate path. The indexed
phase reduces elapsed milliseconds modulo 100,000 before dividing by 1,000,
avoiding a rounding discontinuity at exact 100-second periods. Elapsed seconds
remain OpenEQ's convention, not a verified native time-provider measurement.
`Renderer::render_at` exposes the same render path at a fixed animation time for
reproducible captures and GPU comparisons; interactive rendering uses its clock.

Four independent CPU integration tests verify original Feerrott2's full 65
rectangles/34,132 triangles, pond bounds and tile seams, Buried Sea's 900 sheets
and separate gray selector2, Old Commonlands's lack of active indexed surfaces,
and existing Anguish/finite water mode. Expanded physical triangle counts are
unchanged: Feerrott2 824,400; Buried Sea 1,567,093; Old Commonlands 841,729.

Two GPU tests validate actual patterned normal-map sampling and original terrain
captures. Integer UV repeats, positive/negative world translations, fractional
scale, phase 100 periodicity and the independent coupled shift `du=2/7,dt=100/7`
exercise both normal layers and scroll rates. Legacy water ignores asset UVs as
before. Full pond/seam terrain captures change when their indexed water is
removed. Removing the submerged tile 271 rectangle leaves every pixel identical.

Inspected diagnostic captures are `/tmp/openeq-indexed-water/pond-edge.png`,
`seam.png`, and `occluded-tile 271.png`, with corresponding dry comparisons.
These isolate original terrain/water by omitting static object instances; they
are not whole-zone screenshots or a native-client visual comparison. The existing
Anguish water animation/no-magenta check passes with the new shader. Full
workspace verification is recorded in `OVERNIGHT_2026-09-29.md`.


An explicit six-zone bake/load sweep also completed with no indexed diagnostics:

| Zone | Indexed surfaces | Vertices | Two-sided triangles | Materials |
| --- | ---: | ---: | ---: | ---: |
| Feerrott2 | 65 | 19,932 | 34,132 | 1 |
| Dead Hills | 204 | 22,526 | 34,128 | 1 |
| Loping Plains | 278 | 122,244 | 213,688 | 2 |
| Buried Sea | 900 | 485,304 | 856,856 | 2 |
| Old Commonlands | 0 | 0 | 0 | 0 |
| Nektulos | 0 | 0 | 0 | 0 |

Nektulos retains its existing finite sheets. The sweep asserts that every baked
surface reaches the ordinary loader; its temporary program/output are
`/tmp/openeq-indexed-water-sweep.rs` and `.txt`. It does not establish native
visual parity or gameplay swimming.
