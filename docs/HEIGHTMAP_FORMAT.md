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

Binary EQGZ v2 is also supported. It adds a counted array of u32 values after each placement. The loader consumes that array, preserving following placements/regions/lights. Crescent Reach is the integration fixture.

## Rendering limits and remaining unknowns

This is a compatibility implementation, not a complete reproduction of the client's terrain shader:

- Terrain material layers are composited to a 128×128 RGBA image per tile. This bounds memory for zones with thousands of tiles but loses close-up detail. A GPU terrain material should sample original detail maps and masks directly.
- ECO height/slope ranges and repeats are used, but coverage/blend maps and the client's exact soft blending rules remain unverified. The interpolation is an approximation.
- The two terrain color arrays, MOD/LIT precomputed illumination, ecosystem normal maps, generated radial flora and particle effects are not rendered yet.
- Only finite water sheets are drawn. Per-tile/infinite water and the secondary DAT water record are parsed for stream alignment but their full rendering semantics are not implemented.
- Meanings of the three DAT header words, editor identifiers and quad bits other than bit 0 remain incompletely established. Header bit 1 adds a u32 to individual object records and is handled.
- The binary EQGZ v2 per-placement array is consumed but its lighting interpretation is not yet applied.
- DAT light/effect definitions currently use the first color/intensity frame as a static point light. Temporal effects and exact anchoring should be compared with the original client.

Asset tests verify finite coordinates, valid indices, texture resolution, object counts, mesh topology and parser alignment. They do not claim pixel-identical rendering with the proprietary client.
