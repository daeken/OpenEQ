# Heightmap-zone material count reconciliation

The source-ordinal material fix changes the Feerrott2, Buried Sea, and Arelis
survey totals because previously omitted MOD material groups now load. The
heightmap and indexed-water bakes are unchanged. This note isolates these
three original-asset deltas; it is not a claim about the complete zone survey.
The underlying native identity proof is in
[EQG_MATERIAL_IDENTITY.md](EQG_MATERIAL_IDENTITY.md).

## Exact reconciliation

Scene drawing counts each reusable model definition once. Collision expands
its physical faces through every loaded instance. All restored faces below
have polygon flag bit 0 clear, resolve to ordinary non-water materials, retain
their source winding, and pass the finite/nondegenerate world-space area check
at every authored placement.

| Zone | Original model | Polygon ordinal / stored ID | Restored source triangles | Instances | Added world collision triangles |
| --- | --- | --- | ---: | ---: | ---: |
| Feerrott2 | `obj_feerrott_river_fence_arched.mod` | 7 / 8 | 524 | 1 | 524 |
| Feerrott2 | `obj_feerrott_river_fence_straight.mod` | 7 / 8 | 412 | 4 | 1,648 |
| Buried Sea | `obj_sh_mainmast.mod` | 6 / 8 | 8 | 4 | 32 |
| Arelis | `obj_lunanyn_g.mod` | 6 / 7 | 16 | 1 | 16 |

The former stored-ID map has no key 7 for either fence, and no key 6 for the
mast or Arelis building. Each polygon ordinal is in range of its source record
table. These are sparse-ID failures; no record needs to be overwritten to
trigger this particular bug.

| Zone | Previous drawable triangles | Corrected drawable triangles | Previous world collision | Corrected world collision |
| --- | ---: | ---: | ---: | ---: |
| Feerrott2 | 245,377 | 246,313 (+936) | 824,399 | 826,571 (+2,172) |
| Buried Sea | 1,520,802 | 1,520,810 (+8) | 1,567,097 | 1,567,129 (+32) |
| Arelis | 296,968 | 296,984 (+16) | 1,338,660 | 1,338,676 (+16) |

These match the before/after original-client survey runs
`openeq-native-regions-survey-final` and `openeq-material-bounds-survey-final`.
The regression independently recreates their old counts by removing only the
exact restored source batches and physical triangles from the corrected Scene;
all other terrain, water, models, and placements remain unchanged.

## Source and material witnesses

Offsets are within the uncompressed original MOD member. Every tuple below
is `(a,b,c,material_ordinal,flags)` at the stated polygon index.

| Model | Source material record | Diffuse / normal | Polygon witness |
| --- | --- | --- | --- |
| Arched fence | Record 7, offset `0x6c4`, stored ID 8, `mtl2uv`, `Opaque_MPLBump2UV.fx` | `Di_ent_metal01_c.dds` / `Di_ent_metal01_n.dds` | Polygon 0, offset `0xa360`: `(0,2,3,7,0x20000)` |
| Straight fence | Record 7, offset `0x6c4`, stored ID 8, `mtl2uv`, `Opaque_MPLBump2UV.fx` | `Di_ent_metal01_c.dds` / `Di_ent_metal01_n.dds` | Polygon 0, offset `0x8e68`: `(0,2,3,7,0x20000)` |
| Mainmast | Record 6, offset `0x5be`, stored ID 8, `Material #8979`, `Opaque_MPLBump.fx` | `hp_stumptopa01_c.dds` / `hp_stumptopa01_n.dds` | Polygon 447, offset `0x80be`: `(491,492,493,6,0x20000)` |
| Arelis building | Record 6, offset `0x3ba`, stored ID 7, `mosspanels`, `Opaque_MPLBasic.fx` | `LG_wood_mosspanel_c.dds` / none | Polygon 1,848, offset `0x309ba`: `(927,3136,3137,6,0)` |

Authored loaded instance positions, in OpenEQ's existing coordinate convention:

```text
Arched fence:
  [-1440.0645, -798.6274, -40.670837]
Straight fence:
  [-1400.991,  -868.4441, -40.670837]
  [-1360.9907, -937.7264, -40.670837]
  [-1480.991,  -729.88,   -40.670837]
  [-1520.9908, -660.5981, -40.670837]
Mainmast:
  [4685.9297, 1069.5879, 337.35132]
  [4509.115,  1069.5879, 337.35132]
  [4758.698,   429.8836, 363.60944]
  [4581.884,   429.8836, 363.60944]
Arelis building:
  [2898.3918, -737.97327, -81.80405]
```

The regression checks every restored triangle after the full instance scale,
rotation, and translation, not position alone. It also compares the complete
packed vertices/indices and actual loaded diffuse/normal binding for each
restored drawable group, decodes the diffuse, and compares the exact loaded
physical triangles to the ordered source triangles before removing them.

## Binding changes without added triangles

Arelis building ordinal 7 has 64 triangles. The former ID-7 lookup selected
record 6 (`mosspanels`); ordinal 7 correctly selects record 7, stored ID 8,
`shinmoss` / `LG_roofmossalt_c.dds`. The regression checks its complete unchanged
geometry and corrected Scene diffuse. The archived `obj_lunanyn_g_lod1.mod`
also has 16 missing ordinal-6 triangles and 32 incorrectly bound ordinal-7
triangles, but that LOD is not a loaded object in this Scene, so it contributes
no additional delta.

A bounded archive scan also found Feerrott waterfall material shifts and
Buried Sea `obj_ji_drswtch.mod` material shifts. Their affected references have
both an old and a corrected material, so they do not explain the restored-face
counts above. This note does not claim every archived definition is placed.

## Indexed-water frozen counts

Feerrott2's drawable base changes from 211,245 to 212,181; its 34,132 indexed
water triangles stay unchanged. Its reconstructed old-diagonal collision
baseline changes from 824,400 to 826,572, adding the same 2,172 fence faces.
The measured terrain-anchor rounding delta remains **-1**.

Buried Sea's drawable base changes from 663,946 to 663,954. Its water triangle
sum stays unchanged. Its reconstructed old-diagonal collision baseline changes
from 1,567,093 to 1,567,125, adding the same 32 mast faces. The measured
terrain-anchor rounding delta remains **+4**.

`tests/indexed_water.rs` updates only those frozen base counts. Its exact
surface geometry, selector/material checks, zero-water-collision checks,
legacy-diagonal replay, and individually measured world-area decisions remain
in place.

## Original asset fingerprints

| Archive/member | SHA-256 |
| --- | --- |
| `feerrott2.eqg` | `ec4042de435e7490a4ba970205ef1a6eafe1af8d315a5f2d59cc2c0e07a09eb8` |
| `obj_feerrott_river_fence_arched.mod` | `c26a7596f0217eeeedf5462bb60f4d954587328eb1155ae6b3350c371d4079d9` |
| `obj_feerrott_river_fence_straight.mod` | `6ca5bd99aa50012e3e16f8f89c93bcf93c14379f575d768a1af4c3c21bce4f75` |
| `buriedsea.eqg` | `3809454f0581e5e22f005d84115a1a3c9aac7837bd5507d32ffdfb2b0135df7a` |
| `obj_sh_mainmast.mod` | `3f93469625e02c1fb436b82884bdb25acaac1d3840a777c76853be6a69ca981b` |
| `arelis.eqg` | `b47a93810d232aa5bf233fd42ed3997af685cc6f0c529ef11b5df042d394f67c` |
| `obj_lunanyn_g.mod` | `4e09965eedf5514e02fb283bc96468eb707c9a453fc99be738d6fbc4fb97a1cf` |

The original regression is
`original_heightmap_material_gaps_explain_draw_and_instance_collision_deltas`
in `crates/openeq-assets/tests/eqg_material_identity.rs`. No original binary
assets are exported or committed.
