# DAT region anchors: startup order and first integration candidate

Follow-up to `EQG_LIQUID_TRANSFORMS.md`, 2026-09-29. This records read-only
native control-flow research and original-asset audits. The bounded runtime
integration below uses these startup anchors without changing terrain geometry.

## Startup height anchors use the authored diagonal cache

In the normal client DAT load path, **region height anchoring precedes visible
triangle generation**. Triangle generation itself precedes registered-box
construction, but construction consumes the already stored anchor Z. Therefore
the fact that the flag cache is rebuilt before registration does not mean the
registered boxes were anchored using that rebuilt cache.

Addresses refer to the exact DLL fingerprint in `EQG_LIQUID_TRANSFORMS.md`.
The relevant normal client control flow is:

```text
100a6880  client terrain load / later region registration
  100a6a61 -> 100ec3a0  base terrain load
    100ec4a6 -> grid vtable +3c
      10103e90 -> 10103b60  successful DAT-grid load
        10103be6 -> 1010caf0  load DAT records
        for every loaded tile:
          10103c7a -> tile vtable +130 -> 100a9400
            100a9426 -> 100f4360  base tile initialization
              copy authored heights into tile +68
              bind tile +7c to DAT quad flags (+18)
              100f4485 -> 100f33c0  anchor region composite Z
        for every tile in grid order:
          10103da1 -> 100f2d30  copy neighboring seam heights
        for every tile in grid order:
          10103de3 -> tile vtable +12c -> 100a9560
            100a962c -> 100f6550  generate visible triangles
              clear/rebuild bit80 in the DAT flag cache
              upload the generated indices to D3D
        finish load/cleanup and return
    finish base load and return
  100a6a77 -> 100a40d0  fill terrain grid/collision descriptor
  100a6ae5 onward: register ATP boxes, then other boxes and group regions
```

The concrete vtables make this more than a guessed sequence of unrelated
functions:

- Client terrain constructor `0x100a81d0` installs vtable `0x10140644` at
  `0x100a8247`. Slot `+0x174` is `0x10012950`, which returns false. That
  selects grid constructor `0x10103ab0` in `0x100ec3a0`; its final vtable is
  `0x10144c74`, installed at `0x10103b19`.
- Grid slot `+0x3c` is `0x10103e90`, whose successful path calls
  `0x10103b60` at `0x10103ea8`.
- Client tile vtable `0x101408c4` has `+0x130 = 0x100a9400` and
  `+0x12c = 0x100a9560`.
- Terrain slot `+0x244` is `0x1000c690`, which returns true. Thus the normal
  client chooses tessellator `0x101077d0`, including its row-major grid-index
  remapping, rather than the alternate `0x10107760` path.

Before anchoring, base tile initialization calls tile slots `+0x1c` and
`+0x18`. They resolve to `0x100a8de0 -> 0x100f5c20` and `0x100f5d70`;
these mark material/tile state dirty, not triangle generation. The height copy
is at `0x100f43d0..0x100f4461`, and the flag pointer is assigned at
`0x100f4476..0x100f447b`. Region sampling at `0x100f4485` writes composite
Z through `0x100f33c0`. Later registration adds authored position-Z to that
stored value, rather than calling the sampler again.

The intervening `0x100f2d30` pass copies the positive-X/positive-Y neighbors'
border heights into the tile render grid. It does not resample region anchors.
This is another reason to keep initial anchor reconstruction separate from a
future render-mesh or seam-repair change: even the heights at the outer vertex
row/column can change after initial anchoring.

`0x100a40d0` only fills the terrain grid descriptor; it does not resample
regions. The base-load tail's `0x10106480` frees tessellator scratch storage,
and `0x100a3ce0 -> 0x100ea590` aggregates memory counters. These do not alter
stored anchors either.

### Limits of this ordering proof

This establishes the successful initial DAT load and registration path, not
all editor, height-edit, streaming replacement, or reload operations. There is
a separate refresh function `0x100f3580` that copies heights and re-anchors
regions at `0x100f3651`. Its direct callers include a height-buffer replacement
at `0x100f4125`, a modification path at `0x100fd01d`, and the grid refresh
function `0x101051b0` at `0x10105215`. That grid refresh samples first, then
invokes index generation at `0x10105261`. Later refreshes can therefore consume
the then-current flag cache; treating the original disk flags as eternally
authoritative would be incorrect.

No native DLL execution or live original-client swimming experiment was used.
The CPU helpers reconstruct startup-authored anchors only, within their
documented transform subset. They do not reproduce adaptive terrain LOD.

## Original asset survey

A read-only directory/DAT/TOG audit found 56 installed EQTZP archives, spanning
959 group placements. Only two have no group placements:

| Zone | DAT version | Tiles | Top-level regions | Suitability |
| --- | ---: | ---: | ---: | --- |
| Maiden's Grave (`maidensgrave`) | 21 | 154 | 1 `AWT` | Fits the bounded native box subset |
| Arcstone (`arcstone`) | 20 | 396 | 3 `ATP`, 2 empty names | No authored liquid; one ATP anchor is exactly `[0,0]`, outside the initial subset |

Maiden's Grave is the smallest clear candidate for an initial runtime liquid
integration without decoding embedded group transforms. Its one record is:

```text
DAT offset = 333365
name / alternate = AWT_maiden
type = 1                  native effective type = 5
tile = [7,10]             biased grid = [100007,100010]
quads per tile = 16        vertex spacing = 24       tile width = 384
position = [323.35516357421875, 208.7725372314453, -528.6857299804688]
rotation = [0,0,0]         stored scale = [1,1,1]
full size = [7000,6000,1000]
anchor quad flag = 0x80
height corners h00,h10,h11,h01 =
  [329.3390808105469,322.1661376953125,326.2138366699219,333.2949523925781]
replayed anchor H ~= 328.725709853
float32 stored center = [3011.355224609375,4048.7724609375,-199.96002197265625]
half size = [3500,3000,500]
```

The compiled CPU helper accepts the **complete unmodified Maiden's Grave
Heightmap** as a `NativeTopLevelRegions` set. Its center query is water type 5;
all six inclusive faces and points 0.1 beyond each face were checked. The
large depth is the authored finite box, not a depth inferred from a water
surface. `LiquidRegions::load` now enables this verified whole-zone subset,
retaining native region ordering and suppressing liquid evidence when an
unsupported record wins. Offline movement tests exercise idle depth,
horizontal swimming and surfacing with actual scene collision, plus finite
side entry/exit in an isolated collision world at multiple frame rates.
Server-map compatibility and region-trigger behavior remain separate work;
this slice targets native startup-authored geometry.

### Group metadata can be narrowed, but cannot be assumed empty

All four referenced Feerrott2 TOGs are present and have only object blocks:
`feerott_river_fence.tog`, `feerott_waterfall.tog`,
`feerrott_entrance_pof.tog`, and `feerrott_entrance_thule.tog` contain
5, 5, 28 and 6 objects respectively, with zero `*BEGIN_AREA` blocks. All 12
referenced Loping Plains TOGs and all 8 Old Commonlands TOGs likewise have no
area blocks. A future strict TOG metadata reader could prove that these groups
add no query regions and admit those original zones without guessing embedded
transforms. A missing file must remain unresolved.

The restriction has positive counterexamples. `surefallglade.tog` in Oceangreen
Hills contains one area; `olddranik.tog` contains one; Shard's Landing's single
referenced TOG contains ten. Thus ignoring group regions globally would lose
real authored metadata and can change region precedence.

The survey also found original `ALV` fixtures: Arthicrex has three lava records
(all raw type 0), and Pellucid has six. Arthicrex's sole referenced TOG is
present with no area blocks. These provide original lava cases for a later
strict group-metadata slice. This survey does not establish an original `AVW`
fixture.

Some group files are absent from their zone archive, including all three unique
Dead Hills group names. Loose/shared-archive fallback was not audited here;
absence from one archive must not be mistaken for absence from the native zone.
