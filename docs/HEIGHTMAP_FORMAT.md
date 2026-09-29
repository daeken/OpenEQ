# Heightmap EQG zones

OpenEQ now reads the `EQTZP` terrain format, often called **EQG v4**. This is separate from binary `EQGZ` versions 1 and 2. The format was already partly decoded by [EQEmu zone-utilities](https://github.com/EQEmu/zone-utilities/blob/master/src/common/eqg_v4_loader.cpp); the implementation here was checked against complete files from the installed client, including their final byte offsets.

The parser and CPU mesh conversion are in `openeq-assets::terrain`. Normal zone loading selects this path automatically when the ZON begins with `EQTZP`.

## Confirmed file layout

`*.zon` is whitespace-delimited text. `*NAME` selects the terrain DAT; `*QUADSPERTILE` and `*UNITSPERVERT` define its grid. `*MINLNG/MAXLNG` and `*MINLAT/MAXLAT` delimit editor tiles. Tile coordinates in DAT have a 100000 bias. Longitude is world X, latitude world Y, and elevation world Z. Heights are row-major, with one extra row/column at shared tile boundaries.

The DAT begins with three little-endian u32 words, a NUL-terminated base texture name, and a u32 tile count. Each tile contains:

1. Biased longitude, latitude and an editor identifier (three i32 values).
2. `(quads+1)^2` f32 heights and two arrays of the same length containing u32 colors.
3. `quads^2` flag bytes.
4. A water elevation and a secondary optional water record.
5. Named ecosystem layers. The first has no mask; later layers have a u32 dimension followed by dimension-squared opacity bytes.
6. Counts and records for individual objects, regions, lights/effects and TOG group placements, in that order.

Individual object Z is relative to the triangulated terrain below its local X/Y. Group Z is absolute, plus its stored scale-Z times Z adjustment. Object and group Euler rotations are degrees. TOG files contain local object names, positions, rotations and scales, composed beneath the group transform.

ECO files describe named texture, object and flora sections. Texture layers name `*DETAILMAP` images, repeats and height/slope ranges. The renderer uses the authored ecosystem names and per-tile opacity masks. It preserves both color arrays and all flags in the parsed data. Flag bit 0 suppresses a terrain quad; other flag bits are retained and do not remove visible geometry.

Finite `water.dat` sheets provide extents, elevation, two water colors, normal/environment maps and Fresnel/reflection parameters. Light definitions use packed ARGB colors and intensity; DAT light records provide position and radius.

## Verified client fixtures

Tests parse these DAT streams through the final byte and reject truncation:

| Archive | Tiles | Individual objects | TOG groups | Light/effect records |
| --- | ---: | ---: | ---: | ---: |
| nektulos.eqg | 421 | 2,326 | 67 | 5 |
| oldcommons.eqg | 1,552 | 3,638 | 10 | 0 |
| deadhills.eqg | 1,233 | 942 | 5 | 0 |

`oldcommons.eqg` includes an `oldcommons.zon` declaration whose named DAT is absent, but also contains a complete `commonlands.zon`/`commonlands.dat` pair. The loader resolves the matching declaration rather than inventing a filename. Some Dead Hills TOG files referenced by DAT are absent from this client archive; those missing groups are reported and skipped.

`feerrott2.eqg` contains only `feerrott.zon`/`feerrott.dat`. Exact archive and
loose declarations retain precedence; when both are absent, the loader accepts
an unambiguous archived heightmap declaration naming an existing DAT. Conflicting
alternatives and unreadable candidate declarations fail explicitly. This fixes
an earlier failure looking for `feerrott2.zon` before terrain loading began.
The original fixture now loads 329 textured tiles, 10,901 object instances
and 211,245 terrain/prop source triangles. Indexed water adds 65 authored
rectangles and 34,132 two-sided triangles. Diagnostic pond-edge/seam/occlusion
captures are in `/tmp/openeq-indexed-water`; their object instances are omitted
to expose the original terrain and water. The earlier full-zone overview is
`/tmp/openeq-feerrott2-overview.png`.

Binary EQGZ v2 is also supported. It adds a counted array of u32 values after each placement. The loader consumes that array, preserving following placements/regions/lights. Crescent Reach is the integration fixture.

## Rendering limits and remaining unknowns

This is a compatibility implementation, not a complete reproduction of the client's terrain shader:

- Terrain materials now sample shared original detail images and ordered blend masks directly on the GPU, with filtered mip levels and bounded allocation. Unsupported recipes/devices use the existing clamped 128×128 baked tiles. The current ECO blend interpretation is approximate; native coverage/layering preprocessing remains research. See `GPU_TERRAIN_PLAN.md` and `GPU_TERRAIN_NATIVE_RESEARCH.md`.
- ECO height/slope ranges and repeats are used, but coverage/blend maps and the client's exact soft blending rules remain unverified. The interpolation is an approximation.
- The two terrain color arrays, MOD/LIT precomputed illumination, ecosystem normal maps, generated radial flora and particle effects are not rendered yet.
- Finite sheets and supported indexed tile rectangles are drawn. Indexed surfaces use the native two-sided grid, quantized tile UVs, authored materials and ordinary depth occlusion. The current color/reflection/lighting model remains approximate; the native time provider and complete blend/depth states remain unverified. Native top-level DAT liquid volumes support the verified group-free subset, including swimming in Maiden’s Grave. Embedded group regions, unsupported transforms and binary EQGZ remain unresolved; see `EQG_LIQUID_TRANSFORMS.md`. See `HEIGHTMAP_WATER_SURFACES.md`.
- Native inspection identifies the first DAT header word as the version. The other header words, editor identifiers and quad bits other than bit 0 remain incompletely established. The version-22 object word uses a native `>=22` gate; the current parser's bit test agrees for the audited 20/21/22 layouts, but broader version support needs separate validation.
- The binary EQGZ v2 per-placement array is consumed but its lighting interpretation is not yet applied.
- DAT light/effect definitions currently use the first color/intensity frame as a static point light. Temporal effects and exact anchoring should be compared with the original client.

Asset tests verify finite coordinates, valid indices, texture resolution, object counts, mesh topology and parser alignment. They do not claim pixel-identical rendering with the proprietary client.

## Terrain edge regression

The original Feerrott2 terrain-only view at scene `[-1544,744,60]` looking toward
`[-1544,760,-30]` exposed thin colored tile-grid lines. A synthetic GPU fixture
with different opposite edge colors failed before explicit material clamping.
It now matches a solid-edge reference at all four edges in both opaque and
blended passes; ordinary repeating materials still mix opposite edges. The
alpha-shadow pass uses the same addressing rule.

The original before/after comparison changes 3,932 of 518,400 pixels, leaving
geometry and bounds unchanged. Captures are in `/tmp/openeq-terrain-addressing`.
They deliberately omit object instances to reveal the terrain. This corrects
opposite-edge filtering only; it does not claim seamless authored ecosystems
or original-client terrain shader fidelity.
