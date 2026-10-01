# Binary EQG material identity

TER/MOD polygon references select a **zero-based source material ordinal**,
not the first word stored in a material record. The native material builder
then uses the **first source record with the same exact, case-sensitive name**.
Repeated IDs and names must remain in the parsed source; neither is a safe
map key for preserving authored records.

OpenEQ retains a `Vec<TerMaterial>` in file order, with `stored_id` separately
preserved on each record. `TerMod::material_for_polygon` applies the native
ordinal/name rule for rendering, equipment, and water collision classification.
Polygon references and their material groups remain source ordinals. There is
no fallback to stored ID, material zero, a later name, or a case-folded name.

## Native evidence

Installed `EQGraphicsDX9.dll` SHA-256:
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses use preferred base `0x10000000`.

The TER reader (`0x100643b0`, call at `0x1006445d`) and MOD reader
(`0x10062970`, call at `0x10062a44`) use common routine `0x10062580`:

| Address | Observed operation |
| --- | --- |
| `0x100625b3..0x100625b9` | Walk source material records by `16 + property_count * 12`; stored ID is not used for identity. |
| `0x10062633..0x10062646` | Read polygon word at `+0x0c`, compare unsigned against material count, and increment `counts[reference]` only when in bounds. |
| `0x10062691..0x100626d6` | Iterate used ordinals in ascending order. Walk records to that ordinal and retain its name and raw pointer. |
| `0x100626d9..0x10062748` | Restart at the first record. Compare candidate names byte by byte and stop on the first exact match. Retain its ordinal and pointer separately. |
| `0x10062807..0x10062823` | Select the first-name-match pointer and build the material cache key from caller prefix plus its name. |
| `0x10062845..0x1006289d` | Look up cache type `0x100d`; on miss, pass the canonical raw record and properties to material constructor `0x10014450`. |
| `0x100628bd..0x100628f7` | Rewrite each used source ordinal to a dense runtime slot, install its material, then continue to the next used ordinal. |

The selected ordinal itself always supplies a same-name match in a well-formed
file. An earlier record can supply the material even when no polygon directly
references that earlier ordinal. Runtime material-list caching (`0x100e`) and
material-object sharing are separate from the source record identity; OpenEQ
does not reproduce native resource-cache lifetime behavior here.

Conceptually, before native runtime compaction:

```text
if polygon.material >= records.length:
    no source material
else:
    selected_name = records[polygon.material].name
    material = first record whose name exactly equals selected_name
```

The independent EQEmu zone-utilities reader corroborates ordinal indexing:
`eqg_model_loader.cpp` sizes a material vector to the record count and writes
each parsed record into `mats[i]`. The native instructions above establish the
additional first-name rule.

## Invalid references and the hidden sentinel

The common reader does not put references at or above the source count into
the used-material list. It neither looks them up by stored ID nor substitutes
another material. The runtime accessor `0x10016d20` returns null for a slot at
or above its count (`0x10016d24..0x10016d32`). This alone is not a general render
fallback contract.

The TER upload path calls terrain-manager routine `0x10020970`. It copies the
low 16 bits of each remapped polygon reference into runtime polygon `+2`
(`0x10020b0f..0x10020b14`). The native `0xffff` sentinel has dedicated handling:
`0x10020c4a..0x10020c59` gives it a separate grouping bucket; downstream material
processing skips it at `0x10018494..0x10018499` and
`0x1001869b..0x100186a0`. The latter path otherwise reads a material through
`0x10016d20` and immediately accesses it (`0x100186ab..0x100186b2`), with no null
fallback at that site. These observations do not establish safe behavior for
arbitrary malformed references, especially after native 16-bit truncation.
OpenEQ deliberately does not emulate accidental truncation/aliasing.

The existing OpenEQ bounded policy is unchanged: unresolved ordinals are not
drawable; collision excludes polygon flag bit 0, permits the explicit signed
`-1` material sentinel, and diagnoses/excludes arbitrary unresolved references.
Resolved water uses the canonical material shader. Missing texture data does
not turn a resolved ordinary material into hidden or passable geometry. The
independent collision policy and its EQEmu evidence are recorded in
[EQG_COLLISION_PLAN.md](EQG_COLLISION_PLAN.md); this investigation does not
claim to prove the complete native collision behavior.

## Original Housegarden witness

`housegarden.eqg` SHA-256:
`9b8cdc2c40df7b636dffd514567d4e5e1021e9613545467a5260845a317620f9`.
Member `ter_gardens.ter` SHA-256:
`92a5fa201c3d84e6310c8efc994aea1bdec813d4199c0dc32ef61b256ec7aad5`.

The version-3 payload is 1,843,330 bytes: 4,910 string bytes, 33 material
records, 31,336 vertices, and 22,884 polygons; the polygon table ends exactly
at EOF. It has only 23 distinct stored IDs but 33 distinct names, so the
first-name rule leaves its source ordinals unchanged. The former ID-keyed map
both selected the wrong material and discarded valid drawable groups:

| Material ordinal | Source record offset | Stored ID | Correct material / diffuse | Triangles | Former result |
| ---: | --- | ---: | --- | ---: | --- |
| 3 | `0x1436` | 3 | `stonewall` / `thule_stonestack_c.dds` | 2,210 | Last ID-3 record: ordinal 32, `branchalt` / `branches01.dds`, alpha masked. |
| 23 | `0x192a` | 0 | `brk` / `Di_birch_bark256.dds` | 71 | No ID 23; group dropped. |
| 26 | `0x19f6` | 0 | `stem` / `sp_hedgeA.dds` | 52 | No ID 26; group dropped. |
| 29 | `0x1a4a` | 0 | `bloodbark` / `bloodmoon_bark512_c.dds` | 54 | No ID 29; group dropped. |

The 177 restored triangles raise drawable terrain from 21,531 to 21,708.
Another 1,176 polygons have material `0xffffffff`; they stay outside drawing.
In total 2,387 source triangles had incorrect or absent draw material bindings.

Polygon 0 at `0x1524b2` has indices `[0,1,2]`, material ordinal 3, and flags
`0x20000`. Its authored positions are:

```text
[-251.53598022460938, -321.49957275390625, -38.97163391113281]
[-376.75,            -321.49957275390625, -38.97163391113281]
[-376.75,            -321.49957275390625, -47.74360656738281]
```

Additional retained witnesses are polygon 21,775 at `0x1bc9de` (ordinal 23),
22,608 at `0x1c0af2` (ordinal 26), and 22,660 at `0x1c0f02` (ordinal 29).
The source `Housewall` and `roof` records at ordinals 1 and 2 retain six
additional type-1 channel words that the map previously overwrote. No polygon
references those two records in this TER; restoring the words is source
preservation, not evidence of visible channel support. Linked-zone retained
channel words increase from 138 to 144, as described in
[EQG_MATERIAL_PROPERTIES.md](EQG_MATERIAL_PROPERTIES.md).

## Original Anguish duplicate-name witness

`anguish.eqg` SHA-256:
`b417dbcb0932c8f1fa4a66d7650e587267145125c74fd38f94ec5d97558f2a28`.
Member `ter_island.ter` SHA-256:
`09dfce100dc74bc8d67c37bb8f95e25db500832d9ce718fb99a15fd89be3ce06`.

Records 17 (`0x108e`) and 20 (`0x1106`) both have exact name `prison 13`,
shader `Opaque_MaxCB1.fx`, and diffuse `av_prison13_c.dds`. Their stored IDs
equal their ordinals, so this case isolates name canonicalization:

| Source ordinal | Raw normal map | Polygon count | Native resolved normal map |
| ---: | --- | ---: | --- |
| 17 | `av_rock05_n.dds` | 3,592 | `av_rock05_n.dds` |
| 20 | `av_prison12a_n.dds` | 6,403 | `av_rock05_n.dds` |

Both raw records and properties survive. The 6,403 ordinal-20 triangles now
bind record 17's normal map. `obj_skin.mod` and `obj_stairarch01.mod` contain
the same duplicate records, but no polygons in those two MODs reference them.

## Broad original terrain recovery

The complete installed-zone survey found this identity bug far beyond the
initial Housegarden witness. Independent raw archive parsing exactly reproduces
the old and corrected mesh/triangle totals for these large cases:

| Zone | Drawable triangles before → after | Restored direct terrain triangles | TER records / distinct stored IDs |
| --- | ---: | ---: | ---: |
| Bazaar | 40,067 → 182,661 | 142,594 | 105 / 21 |
| The Nest | 113,397 → 364,385 | 250,988 | 3,813 / 37 |
| Thundercrest | 63,338 → 333,783 | 270,445 | 2,340 / 39 |

All additions in these three zones come from direct terrain. Every restored
polygon has valid finite vertex indices/attributes, and no non-sentinel invalid
material references occur in their declaration-linked models. Hidden material
`0xffffffff` remains excluded from drawing: 24 polygons in Bazaar, none in The
Nest, and 6,834 in Thundercrest (4,798 TER plus 2,036 MOD). This verifies the
restored source geometry; it is not a complete native shader or traversal check.

Representative used ordinals illustrate both stages of material resolution:

- `ter_bazaar.ter` ordinal 65 stores ID 2 and the name `stone`. Its 28,774
  polygons resolve to first-name ordinal 2 and `clz_stone_base_c.dds`.
- `ter_abyss01.ter` ordinal 1126 stores ID 20 and the name `metalwall`. Its
  25,314 polygons resolve to ordinal 20 and `Di_nest_metal_wall_c.dds`.
- `ter_stormtower01.ter` ordinal 1174 stores ID 4 and the name `STORMTOWER_5`.
  Its 22,383 polygons resolve to ordinal 4 and `Di_et_ext_woodplain_c.dds`.

The old map had no entries at those polygon ordinals, even though the authored
material tables did. Raw witnesses, member hashes, sentinel counts and source
polygon offsets are retained in `/tmp/openeq-material-restoration-verify.json`.

## Regressions

`tests/eqg_material_identity.rs` verifies synthetic TER and MOD source record
preservation, repeated/nonordinal IDs, an unused earlier canonical record,
case-sensitive names, exact baked geometry, and both directions of canonical
water classification. Invalid ordinals remain unresolved even if they match
a stored ID; the hidden sentinel stays physical and undrawn.

The opt-in original Housegarden test reconstructs the former ID map, proves
the wrong `branchalt` binding and 177 missing triangles, compares each affected
loaded batch's complete packed vertices and indices against the original
source group, checks its actual Scene diffuse/alpha state, and decodes the
authored texture. The original Anguish test compares both complete source
batches and their loaded diffuse/normal bindings, while checking that the
overridden source normal remains available. Existing Bloodfields frozen draw
fingerprints and physical movement regressions remain unchanged; original
equipment geometry/texture checks also pass. No original asset bytes or native
disassembly dumps are committed.

The heightmap-zone regression additionally matches the exact restored source
batches and instance collision counts for Feerrott2, Buried Sea and Arelis.
Removing only those faces reproduces their previous totals; indexed-water
geometry remains unchanged. See [EQG_MATERIAL_SURVEY.md](EQG_MATERIAL_SURVEY.md).

The GPU regression `openeq-render/tests/eqg_material_identity.rs` uploads the
full original Housegarden scene, then captures source polygons 0–3 at a fixed
camera. The runtime-selected material produces solid stone; an explicit replay
of the former ID-3 lookup produces transparent branch speckles. Both captures
are retained under `/tmp/openeq-housegarden-materials/` and visually inspected.
This verifies the recovered binding through the GPU, without claiming complete
native shader-channel or lighting fidelity.
