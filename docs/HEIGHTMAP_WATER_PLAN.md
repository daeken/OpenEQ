# Indexed heightmap water: Feerrott2 investigation

This began as a read-only CPU investigation of the installed client on 2026-09-29.
The investigation made no runtime, server, GPU, or original asset changes.
It was followed by static analysis of the installed client's terrain reader and
water constructors, recorded in
[HEIGHTMAP_WATER_REVERSE_ENGINEERING.md](HEIGHTMAP_WATER_REVERSE_ENGINEERING.md).
That analysis establishes the version-dependent binary fields and the indexed
mesh's rectangular extent. Complete shoreline rendering, underwater rendering,
and swimming behavior remain unverified.

The lossless metadata/parser/lookup slice is now implemented; see the section
below. It changes no surface geometry, rendering or liquid behavior.

## Findings and confidence

- `*WATERSHEETDATA` is indexed material data, separate from finite
  `*WATERSHEET` rectangles. Feerrott2 has one indexed definition and no finite
  sheet. The previous parser ignored the indexed definition entirely.
- The word after a tile's base water elevation depends on the first DAT
  header word, which the native loader passes as the format version. Version
  20 stores a second float; versions 21/22 store a material selector followed
  by a byte, an optional quartet, and a trailing float. The selector's sign
  does not gate these fields. This supersedes the initial sample-only inference
  of a signed-positive presence gate.
- The following byte controls whether four more floats are present. Every
  quartet audited is an ordered, grid-aligned rectangle inside its tile.
  Coordinate order `[min_local_x, max_local_x, min_local_y, max_local_y]` agrees
  with the existing terrain axes, pond location, submerged terrain, and seams.
  The native reader tests whether the byte is nonzero, including `0x80/0xff`.
- The rectangle is **not exactly reconstructible from current heights**.
  Neither ignoring it nor replacing it with a calculated submerged-quad bound
  is justified. Native constructors pass it to a two-sided subdivided planar
  mesh; it is a geometric extent, not solely cache metadata. The decoded water
  shaders add neither displacement nor shoreline discard. Complete appearance
  and external render states still require comparison.
- The trailing float is -1000 in every extended record audited. Its purpose
  remains unknown. Do not turn it into a liquid bottom or a sentinel rule.

The smallest unconditional change supported by this investigation is a
lossless parser/data-model slice, including indexed material lookup. A small
rectangle-surface experiment is described below, with the additional evidence
needed before calling it compatible rendering.

## Reproducible original fixture

Archive `/Users/daeken/EverQuest/feerrott2.eqg` contains `feerrott.zon` and
`feerrott.dat`: the internal terrain name differs from the archive name.
The DAT parses through byte **2,928,591**, exactly EOF. Header words are
`[21, 0, 12]`. It contains 329 tiles, 10,857 individual placements, four TOG
placements, 11 regions and no point-light/effect records. Tile size is
`16 quads * 16 units = 256` in the current scene coordinate system.

Hashes identify the decompressed entries, without committing their payloads:

| Entry | Bytes | SHA-256 |
| --- | ---: | --- |
| `feerrott.zon` | 258 | `523cdef35961e16d00f5977b369ae428e541eb81be47aa7f45fa7f21705b1da1` |
| `feerrott.dat` | 2,928,591 | `a71e57bebeba8822c059edb3e133056a8f3029a1829809cf5081b1e7dc971820` |
| `water.dat` | 406 | `2e3442645ba98f96bfb296edc6637b596584350d90f466685e5f0baac76da688` |

`water.dat` has one `*WATERSHEETDATA`, `*INDEX 1`, and zero `*WATERSHEET` blocks.
Its material values are:

| Field | Authored value |
| --- | --- |
| Fresnel bias / power | 0.25 / 8 |
| Reflection amount | 0.7 |
| UV scale | 1 |
| Reflection color | `[0.7, 1, 1, 1]` |
| Water color 1 | `[0, 0.04, 0.11, 1]` |
| Water color 2 | `[0, 0.23, 0.17, 1]` |
| Normal / environment map | `Resources\WaterSwap\water_n.dds` / `Resources\WaterSwap\water_e.dds` |

These map names use the existing shared-texture lookup after path stripping.
The native indexed mesh uses tile-relative grid UVs, with `*UVSCALE` supplied
separately as a shader parameter. Reusing the finite-sheet world-coordinate
formula would be an approximation; see the static-analysis document for the
decoded grid lookup, fixed-point vertex packing and two scrolling shader UVs.

## Binary record layout and cross-zone checks

For version 21/22 water records, immediately after the quad-flag array:

| Order | Storage | Feerrott2 contents |
| --- | --- | --- |
| 1 | f32 | Base water elevation: -1000 × 253, -30 × 73, -50 × 3 |
| 2 | i32, preserve raw bits | 1 × 329; exact material-index lookup |
| 3, unconditional | byte, preserved as i8 | 0 × 264, 1 × 65 |
| 4, only if byte != 0 | four f32 | Ordered local rectangle, 65 records |
| 5, unconditional | f32 | -1000 × 329, meaning unresolved |

Version 20 stores only the base elevation and second float. The native reader
at `0x10100dd0` uses the version, not word sign, to distinguish these layouts.
The earlier loader happened to consume all audited fixtures correctly with a
signed-positive word gate, but would desynchronize for a modern zero/negative
selector or legacy positive second float. Native versions below 12 omit these
water fields; this does not establish OpenEQ support for those older complete
DAT layouts. Version-22 object placements have since been traced: their extra
word uses `version >= 22`; a complete version-22 fixture remains unverified.

The following independent complete parses distinguish an index from a simple
record count and guard against making Feerrott2-specific rules universal:

| Zone | DAT header | Bytes through EOF | Tile word distribution | Byte-1 records | Indexed definitions |
| --- | --- | ---: | --- | ---: | --- |
| Feerrott2 | `[21,0,12]` | 2,928,591 | 1 × 329 | 65 | 1 |
| Dead Hills | `[21,0,1]` | 7,985,399 | 1 × 1,233 | 204 | 1 |
| Loping Plains | `[21,0,1]` | 5,203,995 | 1 × 1,057; 2 × 43 | 278 | 1, 2 |
| Buried Sea | `[21,0,20]` | 4,261,992 | 1 × 938; 2 × 23 | 900 | 1, 2 |
| Old Commonlands | `[21,0,1]` | 6,948,252 | 1 × 1,552 | 0 | 83 identical definitions of index 0; one finite sheet |
| Nektulos | `[20,0,10]` | 6,735,104 | `0xc47a0000` × 421 | no extension | No indexed definition; one finite sheet |

Nektulos's word is float -1000 when reinterpreted. Its version-20 record has no
indexed extension, regardless of the second float's sign. The complete native
water-field branch now resolves the ambiguity that these fixtures alone could
not resolve; complete support for other DAT versions is a separate question.

Loping Plains's 43 word-2 tiles still contain **one** extension each and parse
through EOF. Buried Sea's 23 word-2 tiles do the same. A simple record-count
interpretation would lose alignment. Of the active rectangle records,
Loping Plains uses selector 1 × 236 and 2 × 42; Buried Sea uses 1 × 878 and
2 × 22. Buried Sea index 1 has Fresnel bias/power/reflection `0.28/6/0.6`,
while index 2 has `0.15/8/0.3` and different gray water colors. This is strong
asset evidence for the material selector. Subsequent static analysis also found
the native exact-index lookup at `0x100fae00`.

Old Commonlands supplies the necessary counterexample to default/fallback
lookup: all records are byte 0, its DAT selector 1 has no definition, and
83 identical index-0 definitions coexist with a finite sheet. Do not pick the
first material, multiply full-tile surfaces by the definition count, or treat
an unresolved selector as permission to synthesize water. Preserve duplicate
records in the parse; identical duplicates may be coalesced explicitly by a
resolver, while conflicting duplicates should be reported as ambiguous.

## Rectangle evidence and exact examples

All 65 Feerrott2 quartets satisfy `0 <= min < max <= 256` on both axes, with
all coordinates multiples of 16. Eleven cover the entire tile; 54 are partial.
Together their rectangles cover 2,184,448 square world units, or 8,533 grid
quads. The same ordered/grid-aligned property holds for all 204 Dead Hills,
278 Loping Plains, and 900 Buried Sea rectangles, at their respective tile
sizes 128, 224 and 384.

The following positions use **OpenEQ scene axes**, with X from longitude,
Y from latitude and Z upward. Add `(256*longitude, 256*latitude)` to the local
bounds. Do not apply the server's X/Y swap inside this asset loader.

| DAT tile index (zero-based) | Water byte offset | Tile `(lng,lat)` | Local `[xmin,xmax,ymin,ymax]` | Scene `[xmin,xmax,ymin,ymax]` | Z |
| ---: | ---: | --- | --- | --- | ---: |
| 12 | 105,056 | `(-10,-7)` | `[0,256,48,256]` | `[-2560,-2304,-1744,-1536]` | -30 |
| 84 | 732,517 | `(-3,-12)` | `[0,96,112,224]` | `[-768,-672,-2960,-2848]` | -50 |
| 109 | 950,293 | `(-4,-12)` | `[224,256,128,208]` | `[-800,-768,-2944,-2864]` | -50 |
| 235 | 2,017,739 | `(-6,2)` | `[0,256,96,256]` | `[-1536,-1280,608,768]` | -30 |
| 256 | 2,242,152 | `(4,7)` | `[48,256,0,224]` | `[1072,1280,1792,2016]` | -30 |
| 271 | 2,377,367 | `(-7,2)` | `[240,256,240,256]` | `[-1552,-1536,752,768]` | -30 |

At offset 732,517, the complete 29-byte water record is:

```text
00 00 48 c2 | 01 00 00 00 | 01 |
00 00 00 00 | 00 00 c0 42 | 00 00 e0 42 | 00 00 60 43 |
00 00 7a c4
```

That decodes as `-50; 1; 1; [0,96,112,224]; -1000`.
An inactive example is tile 11 `(1,2)`, offset 94,246:
`-30; 1; 0; -1000`, 13 bytes, with terrain entirely above -30
(minimum -29.01466941833496). Thus the base elevation by itself does not
identify a drawable tile.

There are 80 adjacent active-rectangle overlaps at tile seams; every pair
has equal base elevation. Pond tiles 109 and 84 meet at X=-768, with an overlap
Y=-2944 through -2864 at Z=-50. These give a useful future fixed-camera seam
fixture without requiring an inferred liquid volume.

### Why heights cannot replace the stored bounds

A simple audit marks a terrain quad wet if any of its four vertices is strictly
below the base water elevation, then takes the AABB of those quads. This finds
5,515 wet quads. It agrees with the byte's presence on 328/329 tiles and agrees
with the stored rectangle exactly on only **56/65 active tiles**.
The nine exceptions are tile indices `84,109,214,235,243,256,271,275,286`.
Every below-water vertex remains inside the stored rectangle; however, the
surrounding interpolated shoreline can extend beyond that rectangle.

Two concrete counterexamples prevent overclaiming:

- Tile 271 has a recorded 16×16 rectangle at Z=-30, while its **minimum terrain
  height is -12.491677284240723**. No terrain vertex lies under the water.
- Pond tile 84 has recorded maximum local Y=224. At local `(64,224)` terrain
  Z is -50.60579299926758, and at `(64,240)` it is -43.34416198730469.
  A planar interpolation crosses Z=-50 beyond Y=224. The calculated wet-quad
  bound ends at Y=240 instead. A hard crop at the recorded bound removes this
  narrow submerged edge; ignoring the bound preserves it. The native CPU mesh
  uses the stored bound and the decoded water shaders add no shoreline discard.
  Matching final appearance still requires fixed original-client cameras.

The flags also cannot select the water by themselves. Across Feerrott2 the
flag histogram is `{0:52342,128:31855,1:25,4:1,132:1}`. Inside recorded
rectangles it is `{0:4693,128:3840}`; among wet quads it is
`{0:2990,128:2525}`. No hole-bit quads occur inside these particular rectangles,
but that is not a general rule for other zones. Keep bit 128 and do not
interpret all nonzero flags as terrain holes or water masks.

## Region records: useful anchors, separate semantics

The DAT contains six `AWT_*` records and five `ATP_*` records. They are separate
from the surface records, with names, type, repeated grid coordinates,
position, Euler rotation, scale and size. Preserve them before interpreting
liquid volumes. Two exact examples:

| Field | `AWT_small_pond` | `AWT_07_river` |
| --- | --- | --- |
| Byte offset | 738,595 | 1,240,828 |
| Tile | `(-3,-12)` | `(-4,0)` |
| Repeated disk grid | `[99997,99988]` | `[99996,100000]` |
| Type | 0 | 0 |
| Position | `[27.157033920288086,175.78500366210938,-2.802546739578247]` | `[138.54263305664062,128.08828735351562,-73.9330825805664]` |
| Rotation | `[0,0,0]` | `[0,0,0]` |
| Scale | `[1,1,1]` | `[1,1,1]` |
| Size | `[150,100,10]` | `[2000,2500,100]` |

The pond's translated scene XY is approximately `[-740.842966,-2896.214996]`,
inside tile 84's rectangle. The river anchor is approximately
`[-885.457367,128.088287]`. These locate inspection areas; they do not establish
world Z, volume extents, bottom depth, immersion transitions, or swimming.

## Trusted implementations and their limits

Pinned source references were read directly:

- [EQEmu zone-utilities DAT reader](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/common/eqg_v4_loader.cpp#L191)
  reads the word, optional byte/quartet and trailing float using signed-positive
  presence gates, but discards those fields. Those gates match the audited
  assets accidentally; the native version/byte predicates supersede them.
  Its [water parser](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/common/eqg_v4_loader.cpp#L569)
  distinguishes tile definitions and preserves `*INDEX`.
- [EQEmu azone map generation](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/azone/map.cpp#L756)
  emits two noncollidable triangles per **entire tile per indexed definition**
  at the base elevation. It does not use the optional rectangle or select by
  index. This supports a planar surface interpretation, but is a server map
  simplification, not evidence for exact client clipping. Copying it directly
  would duplicate multi-material water and invent inactive surfaces.
- [eqsage's reader](https://github.com/knervous/eqsage/blob/d5dc8328c9e30a88e352ca8a81ccb0efb32bc986/sage/lib/eqg/zone/v4-zone.js#L186)
  consumes and discards the same unknown fields; it supplies no additional
  clipping or selector semantics.
- [Quail's reader](https://github.com/xackery/quail/blob/776fc1acc7676c6984feb6fe0ff9353c4f9e566f/raw/datzon_read.go#L155)
  and writer preserve them as `Unk2`, `Unk3`, `Unk3Quad` and `Unk3Float`.
  These provide comparison data rather than authority over the decoded native
  version branches or rendering behavior.

## Smallest implementation sequence

1. Preserve the tile word's raw bits, optional byte, optional quartet and final
   float. Parse indexed water definitions alongside finite sheets, preserving
   explicit index and all material fields. Keep unresolved/duplicate selectors
   visible to the caller instead of selecting a default. Preserve raw regions
   in the same data-model work only if that is independently scoped; no liquid
   query or movement change is required for surface work.
2. Add synthetic grammar tests for version-21 selectors 1, 2, 0 and negative,
   byte 0/1/127/128/255, positive/negative version-20 second floats,
   truncated optional quartet/tail, duplicate definitions,
   and missing selectors. Add explicitly requested original fixtures for the
   full Feerrott2 alignment/counts, internal-name fallback, exact pond record,
   Loping/Buried selector 2 and Old Commonlands's inactive mismatch.
3. A **candidate**, bounded surface experiment can emit one rectangle per
   resolved nonzero-byte record at its base elevation: Feerrott2 would have 65
   rectangles, 54 partial and 11 full-tile. Use stored bounds, retain existing
   terrain/depth rendering, and mark every surface noncollidable. A simple
   two-triangle rectangle would approximate the native subdivided, two-sided
   mesh. The native UV path is now decoded; follow
   [HEIGHTMAP_WATER_SURFACES.md](HEIGHTMAP_WATER_SURFACES.md) for grid subdivision,
   fixed-point quantization, selector validation and a separate indexed UV mode.
   Do not recompute bounds from height, flood full tiles, use the unknown final
   float as a bottom, or alter camera/player liquid state.
4. Before promoting that candidate to compatibility, compare fixed original-
   client and OpenEQ cameras at pond tile 84's Y=224 edge, submerged-free tile
   271 and the X=-768 pond seam. The CPU rectangle extent is established; these
   comparisons must check final shoreline appearance and continuity. Separately verify
   the selector-2 material appearance in Buried Sea. Above/below screenshots
   should hold exposure, fog, time, camera and UV phase constant where possible.

The current change remains the lossless data/lookup slice. Native mesh evidence
now supports a follow-up surface experiment, but no exact shoreline, swimming,
or full-zone compatibility claim follows from successful DAT alignment or
these metadata correlations.

## Implemented metadata slice

`openeq-assets::terrain` now keeps the post-elevation record in
`TerrainTile::water_metadata`: the original u32 word bits, optional signed tag,
optional four floats and trailing float. Version 21 and newer records always
retain the extension; any nonzero tag includes the quartet. Float bits are
retained without arithmetic, clamping or a meaning being assigned to the
trailing value. `material_index()` returns the signed word, including zero and
negative values, only when an extension is present. Legacy second-float bits
remain available in `word_bits` and do not become material selectors.

`parse_water_data` adds ordered indexed definitions alongside finite sheets.
Indexed records terminate with `*ENDWATERSHEETDATA` (no underscore), within the
`*BEGIN_WATERSHEETDATA` / `*END_WATERSHEETDATA` section wrappers. The parser
retains all field keys and value tokens in order, including unknown/repeated
fields and original texture paths. Whitespace is tokenized, not retained.
Known indexed material fields are decoded without defaults; malformed, missing
or repeated required fields report errors. Finite/indexed blocks cannot nest
inside each other. The existing `parse_water` API and
its finite-sheet defaults/path handling remain unchanged.

`WaterData::resolve_index` reports `Missing`, `Unique` (including the original
occurrence count) or `Ambiguous`. Only definitions equal in all decoded and
authored fields coalesce in the lookup result; the original records are never
removed. No index-zero or first-definition fallback exists. This correctly
keeps Old Commonlands's 83 index-zero records while reporting its tile selector
1 as unresolved.

Synthetic tests cover explicit version-20/21 records, all selector signs,
positive/negative legacy floats, byte values 0/1/127/128/255, exact unusual
float bits, optional-record truncation, text grammar and mixed-block nesting,
unknown fields, duplicate/conflicting definitions and finite-sheet parity. The
explicit original-data test requires every one of the six archives above:

```sh
cargo test -p openeq-assets --test terrain \
  original_six_zones_preserve_indexed_water_metadata_and_lookup -- --ignored --nocapture
```

It verifies full DAT alignment, word/tag/rectangle distributions, indexed
definition counts, the exact Feerrott pond record, selector-2 lookup in Loping
Plains and Buried Sea, the Old Commonlands mismatch, and Nektulos's legacy
record. It performs no surface generation or liquid query. Regions remain
counted but otherwise unpreserved in this independently bounded slice.

Temporary audit programs were `/tmp/heightwater-audit.py`,
`/tmp/heightwater-bounds.py` and `/tmp/heightwater-survey.py`; they read original
archives and wrote only temporary derived metadata. The repository deliverable
of the initial investigation was this document. No original asset payloads are
included in the repository.
