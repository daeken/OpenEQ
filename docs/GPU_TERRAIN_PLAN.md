# Direct terrain textures: asset survey and interface

2026-09-29. This records original-asset measurements and the asset recipe
interface for a bounded direct GPU terrain path. It is a compatibility
improvement over OpenEQ's 128×128 tile bake, **not a recovered native shader**.
The renderer chooses admission and GPU resource limits separately.

## Survey scope and evidence

Read all DAT tile layers/masks and every referenced ECO texture section in
the installed Maiden's Grave, Feerrott2, Old Commonlands, Dead Hills, Loping
Plains and Buried Sea archives. The resulting totals cover 5,329 authored tiles,
5,324 emitted tiles and 2,338 overlay masks. All six DAT versions are 21.
The Rust `terrain_materials` original-asset test independently loads the complete
scenes through the production parser/baker, verifies every emitted recipe and
mask dimension, and successfully decodes every referenced base/detail image.
No proprietary asset bytes are checked in.

The selected declarations are `maidensgrave.zon`, internal `feerrott.zon`,
internal `commonlands.zon`, `deadhills.zon` (`*NAME DeadHills`),
`lopingplains.zon` and `buriedsea.zon`. The existing shared declaration/DAT
resolver is unchanged; the material slice adds no guessed filenames.

| Zone | Emitted/authored tiles | Q / spacing | ECOs | ECO sublayers, min–max | Distinct detail images | Native detail dimensions |
| --- | ---: | --- | ---: | --- | ---: | --- |
| Maiden's Grave | 154/154 | 16 / 24 | 4 | 1–3 | 6 | 6 × 256² |
| Feerrott2 | 329/329 | 16 / 16 | 4 | 1–2 | 5 | 5 × 256² |
| Old Commonlands | 1552/1552 | 16 / 8 | 6 | 1–3 | 5 | 5 × 256² |
| Dead Hills | 1233/1233 | 8 / 16 | 8 | 1–3 | 8 | 3 × 256², 4 × 512², 1 × 1024² |
| Loping Plains | 1100/1100 | 16 / 14 | 5 | 1–3 | 7 | 7 × 256² |
| Buried Sea | 956/961 | 16 / 24 | 7 | 1–3 | 6 | 6 × 256² |

Feerrott2 also has one distinct base image, `di_frott_grass01.dds`, so its full
base/detail source pool is six images. Other resolving base images already
appear among their detail sources. Dead Hills' DAT base string is literally
`None`; it does not resolve, but every tile has a usable first ECO. Missing
optional base alone must not disqualify an otherwise complete recipe.

All named detail images resolve in the zone archive; no loose fallback was
needed in this survey. All are DXT5 DDS files and decode to square RGBA8 images.
The 1024² image is `dh_tower_wall_blocks_c.dds`. Dead Hills has six images with
authored mip counts 9, 10 or 11; its other two and every detail image in the
other five zones have DDS mip count 0 (base level only). Current `Texture`
decoding retains only the base RGBA image even when the DDS contains mips.
Any generated GPU mip chain is therefore an OpenEQ filtering choice.

### Layer and mask distributions

`n:count` below means that many tiles have exactly n DAT ecosystem applications.
The first application has no mask; every later mask is exactly **64×64 bytes**.

| Zone | Tile-layer distribution | Maximum detail sublayers evaluated per tile | Overlay masks | Raw mask bytes | Packed full mip bytes¹ |
| --- | --- | ---: | ---: | ---: | ---: |
| Maiden's Grave | 1:126, 2:28 | 6 | 28 | 114,688 | 152,992 |
| Feerrott2 | 1:141, 2:149, 3:35, 4:4 | 5 | 231 | 946,176 | 1,262,184 |
| Old Commonlands | 1:1377, 2:152, 3:17, 4:2, 5:4 | 12 | 208 | 851,968 | 1,136,512 |
| Dead Hills | 1:340, 2:428, 3:257, 4:176, 5:32 | 9 | 1598 | 6,545,408 | 8,731,472 |
| Loping Plains | 1:877, 2:220, 3:3 | 7 | 226 | 925,696 | 1,234,864 |
| Buried Sea | 1:914, 2:47 | 6 | 47 | 192,512 | 256,808 |

¹ Seven levels 64² through 1², one byte per texel, each level padded to four
bytes for packed-u32 storage: 5,464 bytes per mask. Descriptor overhead is
additional. Counts include all source masks; Buried Sea's five omitted tiles
have no overlay masks.

Every same-zone mask has distinct bytes at its dimension; same-zone hashing
does not currently save memory. Feerrott2 has one all-zero and one all-255
mask; Old Commonlands has one all-zero mask. Every other surveyed mask has
varying values. Content/dimension deduplication remains a useful safe
optimization, but budgets should not assume that it reduces original data.

No referenced ECO was missing or empty, and no consumed numeric field was
malformed, non-finite, reversed or negatively tolerant in these six zones.
Their raw detail repeats range from 2 to 30; height bounds range from -10,000
to 10,000; height tolerances from 1 to 53; slope tolerances from 0 to 40.
Slope bounds legitimately reach **120 degrees** in Feerrott2. Equal endpoints
also occur: Maiden's Grave/Buried Sea use height `[305,305]`, and Loping Plains
uses slope `[0,0]`. These are valid weighted ranges, not malformed intervals.

## Asset interface now provided

```rust
Scene.terrain_materials: BTreeMap<usize, TerrainMaterial>

pub struct TerrainMaterial {
    pub fallback_texture: String,
    pub base_texture: Option<String>,
    pub layers: Vec<MaterialLayer>,
}

pub struct MaterialLayer {
    pub mask_size: usize,
    pub mask: Vec<u8>,
    pub layers: Vec<EcoLayer>,
}
```

Keys are actual emitted material indices, assigned after existing unpainted-tile
and all-hole skips. `fallback_texture` contains the exact existing
`__terrain_<internal-zone-name>_<source-tile-index>.rgba` identity, so a
renderer can reject stale indices after public material replacement/reordering.
Require the current material's sole diffuse name to match that identity and
require ordinary opaque terrain flags before admitting its recipe. The public
map itself cannot detect arbitrary external mutations of `Scene.materials`.

All scene constructors initialize an empty map. Heightmap loading installs the
baker's aligned recipes; object extraction creates a new empty map rather than
carrying stale parent indices into its remapped materials. Material-filtering
tools must clear/remap the metadata with their material arrays.

Each recipe preserves tile-layer order, exact mask dimensions/bytes and ECO
sublayer order/parameters. Base is `Some` only when its image actually decoded.
Missing detail references remain visible in the recipe, but no placeholder
source is inserted. The renderer must reject a recipe requiring an unavailable
image rather than silently drop that sublayer. Empty/unresolved ECO layers
remain empty in their original position and also require an explicit decision.

Native source names are deduplicated case-insensitively before resolution;
each successfully decoded source is moved into `Scene` once and remains
available through `scene.texture(name)`. Existing baked images, material names,
material flags, vertex/index buffers and collision geometry are preserved.
The first slice retains CPU baking; it does not yet reduce zone-load bake time
or CPU memory held by fallback images.

## Current compatibility blend and its limits

These are the existing `terrain/bake.rs` rules, not native shader claims:

1. Start with the DAT base texture at eight repeats per tile, or the current
   missing-base color. Every valid first tile ecosystem replaces it completely.
2. For one ECO, initialize color with its first successfully available detail
   image, regardless of height/slope parameters. Blend later available
   sublayers in their original order. Do not normalize all layer weights.
3. Sample detail textures with repeat bilinear filtering at tile-local UV
   multiplied by `EcoLayer.repeat`.
4. For each later sublayer, alpha is the product of height and slope weights.
   With tolerance t > 0, each range contributes
   `clamp((v-min+t)/t) * clamp((max+t-v)/t)`. With t <= 0 the contribution is
   an inclusive min/max step. Equal endpoints with positive tolerance retain
   a peak; zero tolerance is valid.
5. Composite an ECO result into the tile in DAT order. The first tile layer
   has full opacity; later layers use their own clamp-to-edge bilinear mask.
   A zero-size mask currently returns opacity 1. It must not become division
   by zero in a GPU implementation.

CPU height uses the render mesh's fixed diagonal interpolation. CPU slope uses
`acos(clamp(normal.z,-1,1))` in degrees at the **nearest grid vertex** to a bake
texel. Interpolating per-fragment geometry normals changes transition shapes
and should be described/tested as a rendering improvement, not exact bake
equivalence. The separately recovered native DAT liquid anchor diagonal does
not change these render-height inputs.

The CPU samples normalized encoded RGBA bytes, blends those values, rounds to
RGBA8 and later uploads the tile as sRGB. Sampling native sRGB textures and
then blending linear RGB is a different color-space operation. Choose this
explicitly; compatibility with the current bake requires blending encoded RGB
then converting the final color for the linear lighting buffer. Removing the
128² bake also changes rounding and filtering even under that choice.

ECO sections contain meaningful fields the current parser/baker does not use:
`COVERMAP`, `BLENDMAP`, `BLENDSOFTNESS`, `LAYERINGMAP`, `LAYERINGAREA`,
`NORMALMAP` and `NORMALREPEAT`. These are not all inert defaults. Old
Commonlands includes `grassyblend.bmp`, `blend-grayscale.bmp` and softness
20/50; every surveyed zone names actual coverage and normal images. The two
DAT vertex-color arrays, native seam repair/adaptive topology, normal-map
composition and generated flora remain separate work. Direct detail/mask
sampling alone does not establish native-client appearance.

The current ECO parser is deliberately permissive: malformed numeric tokens
leave defaults, absent fields use defaults, repeat is clamped to at least
0.01, and empty detail declarations are omitted. Other numeric NaN/infinity
can survive parsing. A GPU admission pass must validate finite values,
positive repeats, ordered ranges, compatibility hard-edge behavior for nonpositive tolerances, checked
mask byte lengths and aggregate counts before allocation. Do not clamp valid
120-degree endpoints or reject equal endpoints to simplify the shader.

## Bounded pool recommendation

- Keep recipes keyed by existing material index with the fallback identity
  check above. Validate and admit each entire recipe transactionally; do not
  truncate its layers/sublayers or leave a half-installed mask/detail mapping.
- A first bounded policy allowing up to 8 tile layers, 8 ECO sublayers per
  layer and 32 total detail samples per recipe covers every surveyed recipe.
  These are proposed product limits, not native format maxima. Unsupported
  recipes retain their baked material.
- Keep detail source identity distinct from array placement. All six zones
  need at most 8 base/detail sources, with dimensions no larger than 1024².
  A per-zone array sized to its largest source is feasible here: Dead Hills
  would use 32 MiB at level zero / about 42.7 MiB with RGBA8 mips, versus
  8.75 MiB / about 11.7 MiB at native dimension buckets. A dimension-bucket
  pool saves padding; either approach must budget allocated sizes, not just
  source bytes. Do not downscale the 1024² detail to the existing atlas cell.
- A packed-byte mask storage buffer fits the surveyed maximum in about
  8.33 MiB including all full mip chains. It avoids tying the 1,598-mask
  Dead Hills case to the device's texture-array-layer limit. Store checked
  per-mip offsets/dimensions and extract four R8 values per u32; storing each
  mask byte as its own u32 would quadruple this estimate. Clamp bilinear
  sampling and optional manual trilinear filtering need border/mip tests.
- Derive actual admission from device texture dimensions/array-layer limits,
  storage binding size and total budget, including mip padding and descriptor
  buffers. The measured corpus is not a hard native-format guarantee. A
  prospective 64 MiB detail pool and 16 MiB mask pool cover these six zones,
  but renderer/device policy may choose smaller budgets and fallback.
- Retain the baked texture until admission succeeds; omit its GPU atlas upload
  only for admitted terrain recipes. CPU baked images can be made lazy in a
  later slice after the direct path and fallback behavior are verified.

Asset tests cover skipped-tile alignment, fallback identity, exact mask/source
order, nonstandard but valid ranges, one decode per source, unresolved sources,
object material remapping, and original counts/dimensions for all six zones.
Renderer validation still needs direct-vs-fallback witnesses, mip/wrap edges,
material mutation rejection, budget fallback, original-scene GPU captures and
unchanged collision/topology fingerprints.

## Direct renderer implementation and verification

The renderer now defaults to direct sampling for admitted heightmap scenes.
`GpuScene::build_with_terrain(..., TerrainMode::Baked)` and the diagnostic
`renderzone --baked-terrain` option retain explicit A/B comparison. The source
bake was still prepared on CPU in that checkpoint; the lazy materialization
follow-up below removes it from admitted direct loading.

- Each scene shares a detail array at its largest source dimension (bounded at
  2048), retaining the 1024px Dead Hills detail. Encoded color is filtered/blended
  then decoded to linear for deferred lighting, preserving the compatibility
  color convention. Original alpha never invents terrain transparency.
- Ordered recipes use packed-byte mask mip chains, with clamped bilinear and
  trilinear sampling. First ECO and first tile layers retain their unconditional
  behavior; later ECO weights use rendered height and interpolated normals.
  Zero-opacity and zero-weight details avoid texture fetches.
- The whole scene's recipes are admitted transactionally. Any unsupported
  recipe/source/material identity or device/budget failure selects the baked
  fallback, without a half-installed material map. Aggregate headers/layers/
  masks are preflighted before flattening; detail allocation includes full mip
  padding. Policy is 32 paint/flattened layers per material, 32MiB packed masks,
  and 128MiB combined terrain GPU data, further limited by the device.
- Admitted baked images consume no ordinary atlas layers. Ordinary materials
  sharing a fallback filename still retain their own real atlas mapping. Shadow
  geometry and the water/transparency paths retain their prior behavior.
- Six synthetic GPU tests verify independent reference colors, ordered masks,
  repeats, base-only surfaces, slope/height ranges, tolerance bands, direct and
  mask minification, stale/invalid recipe fallback and shared-name ordinary
  materials in both orders. Four isolated fault injections fail as intended
  when detail/mask mips, height weights or tolerances are disabled.
- Three original-zone GPU comparisons (Feerrott2, Dead Hills, Old Commonlands)
  retain geometry sizes/bounds/draw counts and eliminate baked atlas layers.
  Captures in `/tmp/openeq-gpu-terrain` were inspected. Terrain-only comparisons
  clear placeable instances to expose the ground and water.
- **265 assets/render tests passed**, zero failures/ignored, with original assets
  and GPU. Strict assets/render all-target Clippy passed. Evidence prefix:
  `/tmp/openeq-gpu-terrain-combined-`. The final base-fetch optimization also has
  the seven-test terrain suite in `/tmp/openeq-gpu-terrain-final-tests.log`.

Initial full-scene 960×540 profiling used 60 warmup +120 measured serialized
frames per mode. These are diagnostic runs on the development Mac, not windowed
FPS or a claim of universal speedup. Estimated material allocations include the
ordinary atlas, terrain detail mipmaps and terrain buffers; geometry, actors,
frame targets and sky resources are outside these numbers.

| Zone | Baked material GPU bytes | Direct material GPU bytes | Baked preparation/upload | Direct preparation/upload |
| --- | ---: | ---: | ---: | ---: |
| Feerrott2 | 134,566,740 | 22,982,560 | 827ms | 105ms |
| Dead Hills | 458,575,488 | 81,318,396 | 3,340ms | 429ms |
| Old Commonlands | 574,267,932 | 34,958,864 | 3,494ms | 174ms |

These initial direct runs precede the zero-contribution/base-fetch optimization.
Detail shading performs more work than a single baked lookup: initial median
G-buffer completion intervals increased from 1.49→2.43ms, 0.26→0.49ms and
2.11→2.90ms respectively. Shared-machine/serialized timing noise and overlapping
GPU pass intervals limit comparisons. Asset preparation still took about
1.35/5.64/5.90 seconds; removing its eager fallback paint remains separate work.
Logs and captures use `/tmp/openeq-gpu-terrain/*-profile.*`.

The post-optimization ABBA follow-up used180 measured frames per run. Feerrott2
median G-buffer intervals ranged1.43–3.22ms for baked and1.41–2.82ms for direct;
OldCommons ranged2.73–3.25ms and2.91–3.48ms respectively. The substantial
within-mode variation is evidence against a precise frame-rate claim from these
shared-machine runs. Logs: `*-optimized-*.log` in the same capture directory.

## Lazy compatibility images

Normal zone loading now prepares geometry, shared native sources and immutable
paint recipes. Compatibility tile pixels are painted only when `Scene::texture`
requests them, cached once per tile with `OnceLock`, and returned as owned values.
Source images and paint context are shared across tiles; the public eager `bake`
API and source-name collision ordering remain compatible.

Six new asset tests cover pre-refactor synthetic hashes, 18 independent original
Old Commonlands hashes, missing/malformed sources, name collisions, input
lifetime, object extraction, and concurrent requests. The original loader leaves
all 1,552 Old Commonlands tile caches empty initially. The complete original
asset suite, strict assets Clippy, and all seven terrain GPU tests pass.

Same-machine full-scene checks at 960×540, with direct rendering:

| Zone | Previous asset preparation | Lazy asset preparation | Direct GPU preparation/upload |
| --- | ---: | ---: | ---: |
| Feerrott2 | 1,351ms | 191ms | 100ms |
| Dead Hills | 5,638ms | 130ms | 429ms |
| Old Commonlands | 5,902ms | 120ms | 175ms |

These are individual diagnostic samples, not a benchmark distribution or total
interactive loading time. Forced baked rendering still pays the painting cost,
now during GPU preparation. The Dead Hills and Old Commonlands direct and baked
PNG captures are byte-identical to their pre-refactor counterparts. Feerrott2's
full-scene captures include elapsed-time animation and are not byte-identical;
the original-zone terrain GPU tests use a fixed clock. Allocation, topology,
material counts and bounds remain unchanged. Logs/captures:
`/tmp/openeq-lazy-terrain`; GPU test log:
`/tmp/openeq-lazy-terrain-gpu-tests.log`.
